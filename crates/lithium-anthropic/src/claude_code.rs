//! Claude Code local-state reader.
//!
//! Reads `~/.claude/stats-cache.json` (schema v3) and emits per-day per-model
//! token rows tagged with `source = claude_code_local`. Cost is `None` because
//! Claude Code Max plans are flat-rate; declare them in `[fixed_costs]`.

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use lithium_core::types::{Provider, Source, UsageRow};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use tracing::{debug, info, warn};

/// Reader for `~/.claude/stats-cache.json`.
pub struct ClaudeCodeReader {
    state_dir: PathBuf,
}

impl ClaudeCodeReader {
    /// Construct with the given state directory (typically `~/.claude`).
    pub fn new(state_dir: impl Into<PathBuf>) -> Self {
        Self {
            state_dir: state_dir.into(),
        }
    }

    pub fn stats_cache_path(&self) -> PathBuf {
        self.state_dir.join("stats-cache.json")
    }

    /// True iff the state directory exists. Use this to gracefully skip when
    /// Claude Code is not installed.
    pub fn is_available(&self) -> bool {
        self.state_dir.exists()
    }

    /// Read all per-day per-model token rows from the stats cache.
    ///
    /// Returns one [`UsageRow`] per (date, model) entry in `dailyModelTokens`.
    /// `cost_usd` is `None`; `output_tokens` carries the daily token count
    /// (the source field doesn't break out input/output separately per day).
    pub fn read_daily_token_rows(&self) -> Result<Vec<UsageRow>> {
        if !self.is_available() {
            info!(
                state_dir = %self.state_dir.display(),
                "claude code state dir missing; skipping"
            );
            return Ok(Vec::new());
        }
        let path = self.stats_cache_path();
        if !path.exists() {
            warn!(path = %path.display(), "stats-cache.json missing; skipping");
            return Ok(Vec::new());
        }

        debug!(path = %path.display(), "reading stats-cache");
        let body = std::fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        let parsed: StatsCache = serde_json::from_str(&body)
            .with_context(|| format!("parse stats-cache {}", path.display()))?;

        let mut rows = Vec::new();
        for daily in &parsed.daily_model_tokens {
            let date = NaiveDate::parse_from_str(&daily.date, "%Y-%m-%d")
                .with_context(|| format!("bad date string: {}", daily.date))?;
            let period_start = day_start_utc(date)?;
            let period_end = day_end_utc(date)?;

            for (model, token_count) in &daily.tokens_by_model {
                let raw = serde_json::json!({
                    "stats_cache_version": parsed.version,
                    "date": daily.date,
                    "model": model,
                    "tokens": token_count,
                });
                let row = UsageRow::new_api(
                    Provider::Anthropic,
                    Source::ClaudeCodeLocal,
                    period_start,
                    period_end,
                    raw,
                )
                .with_model(model.clone())
                .with_tokens(0, *token_count, 0, 0); // tokens land in output_tokens; daily breakdown not available
                rows.push(row);
            }
        }
        info!(rows = rows.len(), "claude code rows ready");
        Ok(rows)
    }

    /// Read aggregate per-model token totals (`modelUsage`) as snapshot rows.
    /// Period covers `firstSessionDate` .. `lastComputedDate`. One row per model.
    pub fn read_aggregate_rows(&self) -> Result<Vec<UsageRow>> {
        if !self.is_available() {
            return Ok(Vec::new());
        }
        let path = self.stats_cache_path();
        if !path.exists() {
            return Ok(Vec::new());
        }

        let body = std::fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        let parsed: StatsCache = serde_json::from_str(&body)
            .with_context(|| format!("parse stats-cache {}", path.display()))?;

        let period_start: DateTime<Utc> = parsed
            .first_session_date
            .parse::<DateTime<Utc>>()
            .unwrap_or_else(|_| Utc::now());
        let last_computed_date = NaiveDate::parse_from_str(&parsed.last_computed_date, "%Y-%m-%d")
            .ok()
            .and_then(|d| day_end_utc(d).ok())
            .unwrap_or_else(Utc::now);

        let mut rows = Vec::new();
        for (model, usage) in &parsed.model_usage {
            let raw = serde_json::json!({
                "stats_cache_version": parsed.version,
                "model": model,
                "usage": usage,
                "snapshot_kind": "aggregate"
            });
            let row = UsageRow::new_api(
                Provider::Anthropic,
                Source::ClaudeCodeLocal,
                period_start,
                last_computed_date,
                raw,
            )
            .with_model(format!("{model} (aggregate)"))
            .with_tokens(
                usage.input_tokens,
                usage.output_tokens,
                usage.cache_read_input_tokens,
                usage.cache_creation_input_tokens,
            );
            rows.push(row);
        }
        Ok(rows)
    }
}

fn day_start_utc(date: NaiveDate) -> Result<DateTime<Utc>> {
    let naive = date
        .and_hms_opt(0, 0, 0)
        .context("compose day-start datetime")?;
    Ok(Utc.from_utc_datetime(&naive))
}

fn day_end_utc(date: NaiveDate) -> Result<DateTime<Utc>> {
    let next = date
        .succ_opt()
        .context("date overflow computing next day")?;
    let naive = next
        .and_hms_opt(0, 0, 0)
        .context("compose day-end datetime")?;
    Ok(Utc.from_utc_datetime(&naive))
}

#[derive(Debug, Deserialize)]
struct StatsCache {
    #[serde(default)]
    version: u32,
    #[serde(default, rename = "lastComputedDate")]
    last_computed_date: String,
    #[serde(default, rename = "dailyModelTokens")]
    daily_model_tokens: Vec<DailyModelTokens>,
    #[serde(default, rename = "modelUsage")]
    model_usage: BTreeMap<String, ModelUsage>,
    #[serde(default, rename = "firstSessionDate")]
    first_session_date: String,
}

#[derive(Debug, Deserialize)]
struct DailyModelTokens {
    date: String,
    #[serde(rename = "tokensByModel", default)]
    tokens_by_model: BTreeMap<String, u64>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
    #[serde(default)]
    web_search_requests: u64,
    #[serde(default)]
    cost_usd: f64,
    #[serde(default)]
    context_window: u64,
    #[serde(default)]
    max_output_tokens: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_fixture(dir: &std::path::Path) {
        let stats = serde_json::json!({
            "version": 3,
            "lastComputedDate": "2026-04-22",
            "firstSessionDate": "2026-02-15T18:58:38.686Z",
            "dailyActivity": [],
            "dailyModelTokens": [
                {
                    "date": "2026-04-22",
                    "tokensByModel": {
                        "claude-opus-4-7": 726304
                    }
                },
                {
                    "date": "2026-04-21",
                    "tokensByModel": {
                        "claude-sonnet-4-6": 1404,
                        "claude-haiku-4-5-20251001": 4,
                        "claude-opus-4-7": 59396
                    }
                }
            ],
            "modelUsage": {
                "claude-sonnet-4-6": {
                    "inputTokens": 515810,
                    "outputTokens": 4216195,
                    "cacheReadInputTokens": 540416318,
                    "cacheCreationInputTokens": 39363681,
                    "webSearchRequests": 0,
                    "costUSD": 0.0,
                    "contextWindow": 0,
                    "maxOutputTokens": 0
                }
            },
            "totalSessions": 993,
            "totalMessages": 82765,
            "hourCounts": {}
        });
        fs::write(dir.join("stats-cache.json"), stats.to_string()).unwrap();
    }

    #[test]
    fn missing_dir_is_not_an_error() {
        let reader = ClaudeCodeReader::new("/no/such/dir");
        let rows = reader.read_daily_token_rows().unwrap();
        assert!(rows.is_empty());
        assert!(!reader.is_available());
    }

    #[test]
    fn reads_daily_rows() {
        let tmp = TempDir::new().unwrap();
        write_fixture(tmp.path());
        let reader = ClaudeCodeReader::new(tmp.path());
        let rows = reader.read_daily_token_rows().unwrap();
        // 1 row for 4-22 + 3 rows for 4-21 = 4 total
        assert_eq!(rows.len(), 4);
        let opus_4_22 = rows
            .iter()
            .find(|r| r.model.as_deref() == Some("claude-opus-4-7") && r.period_start.format("%Y-%m-%d").to_string() == "2026-04-22")
            .expect("4-22 opus row");
        assert_eq!(opus_4_22.output_tokens, Some(726304));
        assert!(opus_4_22.cost_usd.is_none(), "no cost for Max plan");
    }

    #[test]
    fn reads_aggregate_rows() {
        let tmp = TempDir::new().unwrap();
        write_fixture(tmp.path());
        let reader = ClaudeCodeReader::new(tmp.path());
        let rows = reader.read_aggregate_rows().unwrap();
        assert_eq!(rows.len(), 1);
        let agg = &rows[0];
        assert_eq!(agg.model.as_deref(), Some("claude-sonnet-4-6 (aggregate)"));
        assert_eq!(agg.input_tokens, Some(515810));
        assert_eq!(agg.cache_read_tokens, Some(540416318));
    }
}
