//! Core domain types shared between adapters, storage, and CLI.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A provider that lithium can poll for usage data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Anthropic,
    OpenAI,
    OpenRouter,
}

impl fmt::Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Provider::Anthropic => write!(f, "anthropic"),
            Provider::OpenAI => write!(f, "openai"),
            Provider::OpenRouter => write!(f, "openrouter"),
        }
    }
}

impl std::str::FromStr for Provider {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "anthropic" => Ok(Provider::Anthropic),
            "openai" => Ok(Provider::OpenAI),
            "openrouter" => Ok(Provider::OpenRouter),
            other => Err(format!("unknown provider: {other}")),
        }
    }
}

/// The specific source within a provider that produced a usage row.
///
/// One provider can have multiple sources (Anthropic has both the Cost Report
/// API and the local Claude Code stats file). Source disambiguates them so we
/// can report each separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Anthropic Admin API cost report (`/v1/organizations/cost_report`)
    AdminApi,
    /// Local `~/.claude/stats-cache.json` parser
    ClaudeCodeLocal,
    /// Anthropic Admin API messages usage report (token counts)
    MessagesUsageReport,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Source::AdminApi => write!(f, "admin_api"),
            Source::ClaudeCodeLocal => write!(f, "claude_code_local"),
            Source::MessagesUsageReport => write!(f, "messages_usage_report"),
        }
    }
}

impl std::str::FromStr for Source {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "admin_api" => Ok(Source::AdminApi),
            "claude_code_local" => Ok(Source::ClaudeCodeLocal),
            "messages_usage_report" => Ok(Source::MessagesUsageReport),
            other => Err(format!("unknown source: {other}")),
        }
    }
}

/// A single usage record for one (provider, source, period, model) tuple.
///
/// This is the canonical row shape stored in the `usage` SQLite table. Adapters
/// produce these; the CLI aggregates and renders them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageRow {
    /// When this row was inserted (set by storage layer at write time).
    pub polled_at: DateTime<Utc>,
    /// Inclusive start of the usage window this row represents.
    pub period_start: DateTime<Utc>,
    /// Exclusive end of the usage window this row represents.
    pub period_end: DateTime<Utc>,
    pub provider: Provider,
    pub source: Source,
    /// Model name. `None` for non-API rows (e.g., aggregate session rows).
    pub model: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_create_tokens: Option<u64>,
    /// Cost in USD. `None` when the source has no $ attribution (Max plans).
    pub cost_usd: Option<f64>,
    /// Session usage percent (0.0-1.0). For Claude Code local source.
    pub session_pct: Option<f64>,
    /// Weekly usage percent (0.0-1.0). For Claude Code local source.
    pub weekly_pct: Option<f64>,
    pub session_resets_at: Option<DateTime<Utc>>,
    pub weekly_resets_at: Option<DateTime<Utc>>,
    /// Raw payload from the source as JSON for debugging.
    pub raw_payload: serde_json::Value,
}

impl UsageRow {
    /// Construct a minimal API-source row. Use the `with_*` helpers to add fields.
    pub fn new_api(
        provider: Provider,
        source: Source,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
        raw_payload: serde_json::Value,
    ) -> Self {
        Self {
            polled_at: Utc::now(),
            period_start,
            period_end,
            provider,
            source,
            model: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_create_tokens: None,
            cost_usd: None,
            session_pct: None,
            weekly_pct: None,
            session_resets_at: None,
            weekly_resets_at: None,
            raw_payload,
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn with_cost_usd(mut self, cost: f64) -> Self {
        self.cost_usd = Some(cost);
        self
    }

    pub fn with_tokens(
        mut self,
        input: u64,
        output: u64,
        cache_read: u64,
        cache_create: u64,
    ) -> Self {
        self.input_tokens = Some(input);
        self.output_tokens = Some(output);
        self.cache_read_tokens = Some(cache_read);
        self.cache_create_tokens = Some(cache_create);
        self
    }
}

/// Status report for a single adapter, returned by `lithium adapters`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterStatus {
    pub provider: Provider,
    pub source: Source,
    pub configured: bool,
    pub last_poll_at: Option<DateTime<Utc>>,
    pub last_poll_status: Option<String>,
    pub last_poll_rows_inserted: Option<u64>,
    pub last_poll_error: Option<String>,
}
