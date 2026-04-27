//! SQLite storage layer.

use crate::error::{Error, Result};
use crate::types::{AdapterStatus, Provider, Source, UsageRow};
use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::{Path, PathBuf};
use tracing::debug;

/// Wrapper around a `rusqlite::Connection` with lithium's schema applied.
pub struct Storage {
    conn: Connection,
    path: PathBuf,
}

impl Storage {
    /// Open or create the SQLite database at the given path. Applies migrations.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        if let Some(parent) = path_buf.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&path_buf)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let storage = Self {
            conn,
            path: path_buf,
        };
        storage.run_migrations()?;
        Ok(storage)
    }

    /// In-memory storage for unit tests.
    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let storage = Self {
            conn,
            path: PathBuf::from(":memory:"),
        };
        storage.run_migrations()?;
        Ok(storage)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn run_migrations(&self) -> Result<()> {
        self.conn.execute_batch(MIGRATION_0001)?;
        Ok(())
    }

    /// Insert or replace a usage row. Idempotent on the
    /// `(provider, source, period_start, period_end, model)` tuple.
    pub fn upsert_usage(&self, row: &UsageRow) -> Result<()> {
        let model_key = row.model.clone().unwrap_or_else(|| "_NULL".to_string());
        debug!(
            provider = %row.provider,
            source = %row.source,
            model = %model_key,
            period_start = %row.period_start,
            "upsert_usage"
        );

        self.conn.execute(
            r#"
            INSERT INTO usage (
                polled_at, period_start, period_end, provider, source, model,
                input_tokens, output_tokens, cache_read_tokens, cache_create_tokens,
                cost_usd, session_pct, weekly_pct, session_resets_at, weekly_resets_at,
                raw_payload
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6,
                ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15,
                ?16
            )
            ON CONFLICT(provider, source, period_start, period_end, model_key)
            DO UPDATE SET
                polled_at = excluded.polled_at,
                input_tokens = excluded.input_tokens,
                output_tokens = excluded.output_tokens,
                cache_read_tokens = excluded.cache_read_tokens,
                cache_create_tokens = excluded.cache_create_tokens,
                cost_usd = excluded.cost_usd,
                session_pct = excluded.session_pct,
                weekly_pct = excluded.weekly_pct,
                session_resets_at = excluded.session_resets_at,
                weekly_resets_at = excluded.weekly_resets_at,
                raw_payload = excluded.raw_payload
            "#,
            params![
                row.polled_at.to_rfc3339(),
                row.period_start.to_rfc3339(),
                row.period_end.to_rfc3339(),
                row.provider.to_string(),
                row.source.to_string(),
                row.model,
                row.input_tokens.map(|v| v as i64),
                row.output_tokens.map(|v| v as i64),
                row.cache_read_tokens.map(|v| v as i64),
                row.cache_create_tokens.map(|v| v as i64),
                row.cost_usd,
                row.session_pct,
                row.weekly_pct,
                row.session_resets_at.map(|d| d.to_rfc3339()),
                row.weekly_resets_at.map(|d| d.to_rfc3339()),
                row.raw_payload.to_string(),
            ],
        )?;
        Ok(())
    }

    /// Record a poll attempt's outcome for `lithium adapters` to display.
    #[allow(clippy::too_many_arguments)]
    pub fn record_poll(
        &self,
        provider: Provider,
        source: Source,
        started_at: DateTime<Utc>,
        finished_at: DateTime<Utc>,
        status: &str,
        rows_inserted: u64,
        error_message: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO poll_log (
                started_at, finished_at, provider, source, status, error_message, rows_inserted
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                started_at.to_rfc3339(),
                finished_at.to_rfc3339(),
                provider.to_string(),
                source.to_string(),
                status,
                error_message,
                rows_inserted as i64,
            ],
        )?;
        Ok(())
    }

    /// Fetch the most recent poll-log entry for a given (provider, source).
    pub fn last_poll(&self, provider: Provider, source: Source) -> Result<Option<PollLogEntry>> {
        let row = self
            .conn
            .query_row(
                r#"
                SELECT started_at, finished_at, status, error_message, rows_inserted
                FROM poll_log
                WHERE provider = ?1 AND source = ?2
                ORDER BY id DESC LIMIT 1
                "#,
                params![provider.to_string(), source.to_string()],
                |r| {
                    Ok(PollLogEntry {
                        started_at: r.get::<_, String>(0)?,
                        finished_at: r.get::<_, Option<String>>(1)?,
                        status: r.get::<_, String>(2)?,
                        error_message: r.get::<_, Option<String>>(3)?,
                        rows_inserted: r.get::<_, i64>(4)? as u64,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    /// Sum cost_usd by (provider, source, model) for a given UTC date window.
    pub fn cost_by_model_in_window(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<DailyCostRow>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT provider, source, COALESCE(model, ''), COALESCE(SUM(cost_usd), 0.0)
            FROM usage
            WHERE period_start >= ?1 AND period_end <= ?2 AND cost_usd IS NOT NULL
            GROUP BY provider, source, model
            ORDER BY provider, source, model
            "#,
        )?;
        let rows = stmt.query_map(
            params![start.to_rfc3339(), end.to_rfc3339()],
            |r| {
                Ok(DailyCostRow {
                    provider: r.get::<_, String>(0)?,
                    source: r.get::<_, String>(1)?,
                    model: r.get::<_, String>(2)?,
                    cost_usd: r.get::<_, f64>(3)?,
                })
            },
        )?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Latest session/weekly limits row for the Claude Code source on a given date.
    pub fn claude_code_limits_on(&self, date: NaiveDate) -> Result<Option<ClaudeCodeLimitRow>> {
        let start = Utc
            .from_utc_datetime(&date.and_hms_opt(0, 0, 0).expect("valid midnight"))
            .to_rfc3339();
        let end = Utc
            .from_utc_datetime(
                &date
                    .succ_opt()
                    .and_then(|d| d.and_hms_opt(0, 0, 0))
                    .expect("valid next midnight"),
            )
            .to_rfc3339();
        let row = self
            .conn
            .query_row(
                r#"
                SELECT session_pct, weekly_pct, session_resets_at, weekly_resets_at
                FROM usage
                WHERE provider = 'anthropic'
                  AND source = 'claude_code_local'
                  AND session_pct IS NOT NULL
                  AND period_start >= ?1 AND period_end <= ?2
                ORDER BY polled_at DESC
                LIMIT 1
                "#,
                params![start, end],
                |r| {
                    Ok(ClaudeCodeLimitRow {
                        session_pct: r.get::<_, Option<f64>>(0)?,
                        weekly_pct: r.get::<_, Option<f64>>(1)?,
                        session_resets_at: r.get::<_, Option<String>>(2)?,
                        weekly_resets_at: r.get::<_, Option<String>>(3)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    /// Get total variable cost in USD for an inclusive UTC date range.
    pub fn total_variable_cost(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<f64> {
        let total: f64 = self.conn.query_row(
            r#"
            SELECT COALESCE(SUM(cost_usd), 0.0)
            FROM usage
            WHERE period_start >= ?1 AND period_end <= ?2 AND cost_usd IS NOT NULL
            "#,
            params![start.to_rfc3339(), end.to_rfc3339()],
            |r| r.get(0),
        )?;
        Ok(total)
    }

    /// Build adapter status from the most recent poll-log entry per (provider, source).
    pub fn adapter_status(
        &self,
        provider: Provider,
        source: Source,
        configured: bool,
    ) -> Result<AdapterStatus> {
        let last = self.last_poll(provider, source)?;
        Ok(AdapterStatus {
            provider,
            source,
            configured,
            last_poll_at: last
                .as_ref()
                .map(|p| {
                    DateTime::parse_from_rfc3339(&p.started_at)
                        .map(|d| d.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now())
                }),
            last_poll_status: last.as_ref().map(|p| p.status.clone()),
            last_poll_rows_inserted: last.as_ref().map(|p| p.rows_inserted),
            last_poll_error: last.and_then(|p| p.error_message),
        })
    }

    /// Total spend grouped by date in the given UTC window. Used for projection.
    pub fn daily_costs_in_month(&self, year: i32, month: u32) -> Result<Vec<(NaiveDate, f64)>> {
        let start = NaiveDate::from_ymd_opt(year, month, 1)
            .ok_or_else(|| Error::Storage(format!("invalid month: {year}-{month}")))?;
        let next_month = if month == 12 {
            NaiveDate::from_ymd_opt(year + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(year, month + 1, 1)
        }
        .ok_or_else(|| Error::Storage(format!("invalid next-month boundary: {year}-{month}")))?;

        let start_utc = Utc.from_utc_datetime(&start.and_hms_opt(0, 0, 0).unwrap());
        let end_utc = Utc.from_utc_datetime(&next_month.and_hms_opt(0, 0, 0).unwrap());

        let mut stmt = self.conn.prepare(
            r#"
            SELECT substr(period_start, 1, 10) AS day, COALESCE(SUM(cost_usd), 0.0)
            FROM usage
            WHERE period_start >= ?1 AND period_end <= ?2 AND cost_usd IS NOT NULL
            GROUP BY day
            ORDER BY day
            "#,
        )?;
        let rows = stmt.query_map(
            params![start_utc.to_rfc3339(), end_utc.to_rfc3339()],
            |r| {
                let day_str: String = r.get(0)?;
                let cost: f64 = r.get(1)?;
                Ok((day_str, cost))
            },
        )?;
        let mut out = Vec::new();
        for row in rows {
            let (day_str, cost) = row?;
            let date = NaiveDate::parse_from_str(&day_str, "%Y-%m-%d").map_err(|e| {
                Error::Storage(format!("bad date string in db: {day_str} ({e})"))
            })?;
            out.push((date, cost));
        }
        Ok(out)
    }

    /// Current Y-M for the calendar bucket the user lives in. Pure helper.
    pub fn current_year_month_utc(&self) -> (i32, u32) {
        let now = Utc::now();
        (now.year(), now.month())
    }
}

#[derive(Debug, Clone)]
pub struct PollLogEntry {
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub rows_inserted: u64,
}

#[derive(Debug, Clone)]
pub struct DailyCostRow {
    pub provider: String,
    pub source: String,
    pub model: String,
    pub cost_usd: f64,
}

#[derive(Debug, Clone)]
pub struct ClaudeCodeLimitRow {
    pub session_pct: Option<f64>,
    pub weekly_pct: Option<f64>,
    pub session_resets_at: Option<String>,
    pub weekly_resets_at: Option<String>,
}

const MIGRATION_0001: &str = r#"
CREATE TABLE IF NOT EXISTS usage (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    polled_at           TEXT NOT NULL,
    period_start        TEXT NOT NULL,
    period_end          TEXT NOT NULL,
    provider            TEXT NOT NULL,
    source              TEXT NOT NULL,
    model               TEXT,
    -- model_key is a generated NOT-NULL column we use for the unique index;
    -- SQLite cannot enforce uniqueness when one of the columns is NULL, so we
    -- substitute '_NULL' for missing model values to keep idempotency clean.
    model_key           TEXT GENERATED ALWAYS AS (COALESCE(model, '_NULL')) STORED,
    input_tokens        INTEGER,
    output_tokens       INTEGER,
    cache_read_tokens   INTEGER,
    cache_create_tokens INTEGER,
    cost_usd            REAL,
    session_pct         REAL,
    weekly_pct          REAL,
    session_resets_at   TEXT,
    weekly_resets_at    TEXT,
    raw_payload         TEXT NOT NULL,
    UNIQUE (provider, source, period_start, period_end, model_key)
);

CREATE INDEX IF NOT EXISTS idx_usage_period_start ON usage(period_start);
CREATE INDEX IF NOT EXISTS idx_usage_provider_source ON usage(provider, source);

CREATE TABLE IF NOT EXISTS poll_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at      TEXT NOT NULL,
    finished_at     TEXT,
    provider        TEXT NOT NULL,
    source          TEXT NOT NULL,
    status          TEXT NOT NULL,
    error_message   TEXT,
    rows_inserted   INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_poll_log_provider_source ON poll_log(provider, source);
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use serde_json::json;

    fn sample_row(model: &str, cost: f64, day: i64) -> UsageRow {
        let start = Utc::now() - Duration::days(day);
        let end = start + Duration::days(1);
        UsageRow::new_api(
            Provider::Anthropic,
            Source::AdminApi,
            start,
            end,
            json!({"sample": true}),
        )
        .with_model(model)
        .with_cost_usd(cost)
    }

    #[test]
    fn upsert_is_idempotent() {
        let s = Storage::open_in_memory().unwrap();
        let r1 = sample_row("claude-sonnet-4-6", 1.23, 0);
        let r2 = sample_row("claude-sonnet-4-6", 4.56, 0);
        // Second one has same period+model+source so should REPLACE not duplicate.
        // Make periods exactly identical for the test:
        let mut r2 = r2;
        r2.period_start = r1.period_start;
        r2.period_end = r1.period_end;

        s.upsert_usage(&r1).unwrap();
        s.upsert_usage(&r2).unwrap();

        let rows: Vec<_> = s
            .conn
            .prepare("SELECT cost_usd FROM usage")
            .unwrap()
            .query_map([], |r| r.get::<_, f64>(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        assert_eq!(rows.len(), 1, "upsert should not duplicate");
        assert!((rows[0] - 4.56).abs() < 1e-9, "later row should win");
    }

    #[test]
    fn poll_log_records_attempt() {
        let s = Storage::open_in_memory().unwrap();
        let now = Utc::now();
        s.record_poll(Provider::Anthropic, Source::AdminApi, now, now, "ok", 5, None)
            .unwrap();
        let last = s
            .last_poll(Provider::Anthropic, Source::AdminApi)
            .unwrap()
            .unwrap();
        assert_eq!(last.status, "ok");
        assert_eq!(last.rows_inserted, 5);
    }

    #[test]
    fn cost_by_model_groups_correctly() {
        let s = Storage::open_in_memory().unwrap();

        // Use deterministic, well-separated periods so the window query catches all of them.
        let day0_start = Utc.with_ymd_and_hms(2026, 4, 25, 0, 0, 0).unwrap();
        let day0_end = Utc.with_ymd_and_hms(2026, 4, 26, 0, 0, 0).unwrap();
        let day1_start = Utc.with_ymd_and_hms(2026, 4, 26, 0, 0, 0).unwrap();
        let day1_end = Utc.with_ymd_and_hms(2026, 4, 27, 0, 0, 0).unwrap();

        let a = UsageRow::new_api(
            Provider::Anthropic,
            Source::AdminApi,
            day0_start,
            day0_end,
            json!({}),
        )
        .with_model("claude-sonnet-4-6")
        .with_cost_usd(1.0);
        let b = UsageRow::new_api(
            Provider::Anthropic,
            Source::AdminApi,
            day1_start,
            day1_end,
            json!({}),
        )
        .with_model("claude-sonnet-4-6")
        .with_cost_usd(2.0);
        let c = UsageRow::new_api(
            Provider::Anthropic,
            Source::AdminApi,
            day0_start,
            day0_end,
            json!({}),
        )
        .with_model("claude-haiku-4-5")
        .with_cost_usd(0.5);

        s.upsert_usage(&a).unwrap();
        s.upsert_usage(&b).unwrap();
        s.upsert_usage(&c).unwrap();

        let window_start = Utc.with_ymd_and_hms(2026, 4, 24, 0, 0, 0).unwrap();
        let window_end = Utc.with_ymd_and_hms(2026, 4, 28, 0, 0, 0).unwrap();
        let rows = s.cost_by_model_in_window(window_start, window_end).unwrap();
        assert_eq!(rows.len(), 2, "two distinct models");
        let sonnet = rows
            .iter()
            .find(|r| r.model == "claude-sonnet-4-6")
            .unwrap();
        assert!(
            (sonnet.cost_usd - 3.0).abs() < 1e-9,
            "sonnet should sum to 3.0, got {}",
            sonnet.cost_usd
        );
        let haiku = rows.iter().find(|r| r.model == "claude-haiku-4-5").unwrap();
        assert!((haiku.cost_usd - 0.5).abs() < 1e-9);
    }
}
