//! lithium-openrouter: OpenRouter adapter
//!
//! Hits `GET https://openrouter.ai/api/v1/key`. The endpoint accepts any
//! regular OpenRouter API key (no admin / management key required) and
//! pre-aggregates `usage_daily`, `usage_weekly`, `usage_monthly` for the
//! authenticated key — so we just write today's row per poll, no
//! snapshot-delta math.
//!
//! See `docs/SPEC-PHASE-1.md` (Phase 2 section in successor docs).

use anyhow::{Context, Result};
use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use lithium_core::types::{Provider, Source, UsageRow};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

const KEY_ENDPOINT_PATH: &str = "/api/v1/key";

pub struct OpenRouterClient {
    client: Client,
    base_url: String,
    api_key: String,
}

impl OpenRouterClient {
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Self::with_base_url(api_key, "https://openrouter.ai")
    }

    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Result<Self> {
        let client = Client::builder()
            .user_agent(concat!("lithium/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .context("build reqwest client")?;
        Ok(Self {
            client,
            base_url: base_url.into(),
            api_key: api_key.into(),
        })
    }

    /// Fetch the /api/v1/key response and return up to two UsageRows:
    /// one for today's spend (period = today UTC) and one for month-to-date
    /// (period = month-start UTC -> today end UTC), both with the same data
    /// for cross-checking. The CLI only sums daily rows in `lithium today`,
    /// and the month-to-date row is informational for `lithium adapters`.
    pub async fn fetch_usage(&self) -> Result<Vec<UsageRow>> {
        let url = format!("{}{}", self.base_url, KEY_ENDPOINT_PATH);
        debug!(url = %url, "GET openrouter key");

        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.api_key)
            .send()
            .await
            .context("openrouter /api/v1/key request")?;

        let status = resp.status();
        let body_text = resp.text().await.context("read response body")?;

        if !status.is_success() {
            if status.as_u16() == 401 {
                anyhow::bail!(
                    "401 Unauthorized from OpenRouter /api/v1/key. \
                     Hint: regenerate the key at openrouter.ai/keys and update \
                     ~/.config/lithium/config.toml"
                );
            }
            anyhow::bail!(
                "openrouter /api/v1/key returned HTTP {}: {}",
                status.as_u16(),
                truncate(&body_text, 500)
            );
        }

        // OpenRouter returns either {data: {...}} or the fields directly,
        // depending on docs version. Handle both.
        let parsed: KeyResponse = serde_json::from_str::<KeyEnvelope>(&body_text)
            .map(|e| e.data)
            .or_else(|_| serde_json::from_str::<KeyResponse>(&body_text))
            .with_context(|| {
                format!("parse openrouter key body; raw={}", truncate(&body_text, 500))
            })?;

        info!(
            label = %parsed.label.as_deref().unwrap_or(""),
            usage_daily = parsed.usage_daily,
            usage_monthly = parsed.usage_monthly,
            "openrouter usage parsed"
        );

        let now = Utc::now();
        let today = now.date_naive();
        let raw = serde_json::to_value(&parsed).unwrap_or(serde_json::Value::Null);

        // Use a short stable model label so the today/month per-row column
        // doesn't wrap. Key label moves into raw_payload for forensics.
        let today_row = UsageRow::new_api(
            Provider::OpenRouter,
            Source::AdminApi,
            day_start_utc(today),
            day_end_utc(today),
            raw.clone(),
        )
        .with_model("openrouter (daily)")
        .with_cost_usd(parsed.usage_daily);

        Ok(vec![today_row])
    }
}

#[derive(Debug, Deserialize)]
struct KeyEnvelope {
    data: KeyResponse,
}

#[derive(Debug, Deserialize, Serialize)]
struct KeyResponse {
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    limit: Option<f64>,
    #[serde(default)]
    limit_remaining: Option<f64>,
    #[serde(default)]
    is_free_tier: bool,

    /// Total OpenRouter credit usage (in USD) for the API key, all-time.
    #[serde(default)]
    usage: f64,
    /// USD spend in the current UTC day.
    #[serde(default)]
    usage_daily: f64,
    /// USD spend in the current UTC week (Monday-Sunday).
    #[serde(default)]
    usage_weekly: f64,
    /// USD spend in the current UTC month.
    #[serde(default)]
    usage_monthly: f64,
}

fn day_start_utc(d: NaiveDate) -> chrono::DateTime<Utc> {
    Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).expect("valid midnight"))
}

fn day_end_utc(d: NaiveDate) -> chrono::DateTime<Utc> {
    let next = d.succ_opt().expect("date can advance one day");
    Utc.from_utc_datetime(&next.and_hms_opt(0, 0, 0).expect("valid midnight"))
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…[truncated, {} chars]", &s[..max], s.len() - max)
    }
}

/// Suppress unused warning for Datelike if not used elsewhere.
#[allow(dead_code)]
fn _datelike_anchor(d: chrono::DateTime<Utc>) -> i32 {
    d.year()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn parses_envelope_form() {
        let server = MockServer::start().await;
        let body = serde_json::json!({
            "data": {
                "label": "My Default Key",
                "limit": 500.0,
                "limit_remaining": 450.0,
                "is_free_tier": false,
                "usage": 50.0,
                "usage_daily": 5.0,
                "usage_weekly": 10.0,
                "usage_monthly": 20.0
            }
        });
        Mock::given(method("GET"))
            .and(path("/api/v1/key"))
            .and(header("authorization", "Bearer sk-or-FAKE"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let client = OpenRouterClient::with_base_url("sk-or-FAKE", server.uri()).unwrap();
        let rows = client.fetch_usage().await.unwrap();
        assert_eq!(rows.len(), 1);
        let r = &rows[0];
        assert_eq!(r.provider, Provider::OpenRouter);
        assert!((r.cost_usd.unwrap() - 5.0).abs() < 1e-9);
        assert_eq!(r.model.as_deref(), Some("openrouter (daily)"));
    }

    #[tokio::test]
    async fn parses_flat_form() {
        let server = MockServer::start().await;
        let body = serde_json::json!({
            "label": "Flat Key",
            "limit": null,
            "limit_remaining": null,
            "is_free_tier": false,
            "usage": 12.0,
            "usage_daily": 1.5,
            "usage_weekly": 4.0,
            "usage_monthly": 12.0
        });
        Mock::given(method("GET"))
            .and(path("/api/v1/key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let client = OpenRouterClient::with_base_url("sk-or-FAKE", server.uri()).unwrap();
        let rows = client.fetch_usage().await.unwrap();
        assert_eq!(rows.len(), 1);
        assert!((rows[0].cost_usd.unwrap() - 1.5).abs() < 1e-9);
    }

    #[tokio::test]
    async fn unauthorized_surfaces_clear_hint() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/key"))
            .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
            .mount(&server)
            .await;

        let client = OpenRouterClient::with_base_url("sk-or-BAD", server.uri()).unwrap();
        let err = client.fetch_usage().await.err().unwrap();
        let msg = format!("{err:#}");
        assert!(msg.contains("401"));
        assert!(msg.contains("openrouter.ai/keys"));
    }
}
