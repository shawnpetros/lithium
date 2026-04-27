//! Anthropic Admin API client.
//!
//! Endpoint: `GET https://api.anthropic.com/v1/organizations/cost_report`
//! Auth: `X-Api-Key: <admin_api_key>` (admin keys, NOT regular API keys)
//!     + `anthropic-version: 2023-06-01`
//!
//! We poll with `bucket_width=1d` and `group_by=description` to get per-model
//! cost breakdown. Pagination is honored via `next_page` / `has_more`.

use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use lithium_core::types::{Provider, Source, UsageRow};
use reqwest::{Client, header};
use serde::Deserialize;
use tracing::{debug, info, warn};

const COST_REPORT_PATH: &str = "/v1/organizations/cost_report";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic Admin API client.
///
/// Construct with [`AdminApiClient::new`] for production use, or
/// [`AdminApiClient::with_base_url`] in tests against `wiremock`.
pub struct AdminApiClient {
    client: Client,
    base_url: String,
    admin_key: String,
}

impl AdminApiClient {
    /// Build a client targeting api.anthropic.com.
    pub fn new(admin_key: impl Into<String>) -> Result<Self> {
        Self::with_base_url(admin_key, "https://api.anthropic.com")
    }

    /// Build a client against a custom base URL (for tests).
    pub fn with_base_url(admin_key: impl Into<String>, base_url: impl Into<String>) -> Result<Self> {
        let client = Client::builder()
            .user_agent(concat!("lithium/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("build reqwest client")?;
        Ok(Self {
            client,
            base_url: base_url.into(),
            admin_key: admin_key.into(),
        })
    }

    /// Fetch the cost report for the given UTC date range, paginating through.
    ///
    /// `starting_at` is required by the API; `ending_at` is optional (we always
    /// supply it to bound the query). Both are RFC3339-formatted at minute
    /// boundaries; the API snaps them to bucket boundaries internally.
    pub async fn fetch_cost_report(
        &self,
        starting_at: DateTime<Utc>,
        ending_at: DateTime<Utc>,
    ) -> Result<Vec<UsageRow>> {
        let mut rows: Vec<UsageRow> = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let mut req = self
                .client
                .get(format!("{}{}", self.base_url, COST_REPORT_PATH))
                .header(header::HeaderName::from_static("x-api-key"), &self.admin_key)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .query(&[
                    ("starting_at", starting_at.to_rfc3339()),
                    ("ending_at", ending_at.to_rfc3339()),
                    ("bucket_width", "1d".into()),
                ])
                // group_by accepts repeated values; reqwest serializes Vec<(k, v)> with
                // duplicated keys, which is what the API expects for array params.
                .query(&[("group_by[]", "description")]);

            if let Some(token) = &page_token {
                req = req.query(&[("page", token.as_str())]);
            }

            debug!(starting_at = %starting_at, ending_at = %ending_at, "GET cost_report");
            let resp = req.send().await.context("cost_report request")?;
            let status = resp.status();
            let body_text = resp.text().await.context("read response body")?;

            if !status.is_success() {
                if status.as_u16() == 401 {
                    anyhow::bail!(
                        "401 Unauthorized from Anthropic Admin API. \
                         Hint: regenerate the admin key at console.anthropic.com -> \
                         Settings -> Admin Keys, then update ~/.config/lithium/config.toml"
                    );
                }
                if status.as_u16() == 429 {
                    anyhow::bail!(
                        "429 rate-limited by Anthropic Admin API; try again later. body={body_text}"
                    );
                }
                anyhow::bail!(
                    "cost_report returned HTTP {}: {}",
                    status.as_u16(),
                    truncate(&body_text, 500)
                );
            }

            let parsed: CostReportResponse = serde_json::from_str(&body_text)
                .with_context(|| format!("parse cost_report body; raw={}", truncate(&body_text, 500)))?;

            let mut bucket_count = 0usize;
            for bucket in &parsed.data {
                bucket_count += 1;
                let bucket_start = bucket.starting_at;
                let bucket_end = bucket.ending_at;
                for result in &bucket.results {
                    if let Some(row) = cost_result_to_usage_row(bucket_start, bucket_end, result) {
                        rows.push(row);
                    }
                }
            }
            info!(
                buckets = bucket_count,
                page_more = parsed.has_more,
                "cost_report page processed"
            );

            if parsed.has_more {
                page_token = parsed.next_page;
                if page_token.is_none() {
                    warn!("has_more=true but next_page missing; breaking pagination loop");
                    break;
                }
            } else {
                break;
            }
        }

        Ok(rows)
    }

    /// Convenience: fetch month-to-date cost report.
    pub async fn fetch_month_to_date(&self, today_utc: DateTime<Utc>) -> Result<Vec<UsageRow>> {
        let month_start = NaiveDate::from_ymd_opt(today_utc.year(), today_utc.month(), 1)
            .context("invalid month start")?
            .and_hms_opt(0, 0, 0)
            .context("invalid hms")?;
        let starting_at = Utc.from_utc_datetime(&month_start);
        let ending_at = today_utc + chrono::Duration::days(1);
        self.fetch_cost_report(starting_at, ending_at).await
    }
}

#[derive(Debug, Deserialize)]
struct CostReportResponse {
    data: Vec<CostBucket>,
    has_more: bool,
    #[serde(default)]
    next_page: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CostBucket {
    starting_at: DateTime<Utc>,
    ending_at: DateTime<Utc>,
    results: Vec<CostResult>,
}

#[derive(Debug, Deserialize, Clone)]
struct CostResult {
    /// Cost as a decimal string in cents (e.g. "123.45" = $1.2345 → divide by 100).
    amount: String,
    #[serde(default)]
    currency: Option<String>,
    #[serde(default)]
    cost_type: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    token_type: Option<String>,
    #[serde(default)]
    service_tier: Option<String>,
    #[serde(default)]
    context_window: Option<String>,
    #[serde(default)]
    workspace_id: Option<String>,
}

fn cost_result_to_usage_row(
    period_start: DateTime<Utc>,
    period_end: DateTime<Utc>,
    result: &CostResult,
) -> Option<UsageRow> {
    let cents = result.amount.parse::<f64>().ok()?;
    let cost_usd = cents / 100.0;

    let raw = serde_json::to_value(result).ok()?;

    // The model name comes through directly when group_by=description.
    // For non-token cost_types (web_search, code_execution, session_usage), no model is set,
    // so we synthesize a label so the row is still attributable.
    let model_label = result.model.clone().or_else(|| match result.cost_type.as_deref() {
        Some("web_search") => Some("anthropic.web_search".to_string()),
        Some("code_execution") => Some("anthropic.code_execution".to_string()),
        Some("session_usage") => Some("anthropic.claude_code_session".to_string()),
        Some("tokens") => result.description.clone(),
        _ => result.description.clone(),
    });

    Some(
        UsageRow::new_api(Provider::Anthropic, Source::AdminApi, period_start, period_end, raw)
            .maybe_with_model(model_label)
            .with_cost_usd(cost_usd),
    )
}

trait UsageRowExt {
    fn maybe_with_model(self, model: Option<String>) -> Self;
}

impl UsageRowExt for UsageRow {
    fn maybe_with_model(mut self, model: Option<String>) -> Self {
        if let Some(m) = model {
            self.model = Some(m);
        }
        self
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…[truncated, {} chars]", &s[..max], s.len() - max)
    }
}

impl serde::Serialize for CostResult {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("CostResult", 9)?;
        s.serialize_field("amount", &self.amount)?;
        s.serialize_field("currency", &self.currency)?;
        s.serialize_field("cost_type", &self.cost_type)?;
        s.serialize_field("description", &self.description)?;
        s.serialize_field("model", &self.model)?;
        s.serialize_field("token_type", &self.token_type)?;
        s.serialize_field("service_tier", &self.service_tier)?;
        s.serialize_field("context_window", &self.context_window)?;
        s.serialize_field("workspace_id", &self.workspace_id)?;
        s.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn sample_response_body() -> serde_json::Value {
        serde_json::json!({
            "data": [
                {
                    "starting_at": "2026-04-25T00:00:00Z",
                    "ending_at": "2026-04-26T00:00:00Z",
                    "results": [
                        {
                            "amount": "1234.50",
                            "currency": "USD",
                            "cost_type": "tokens",
                            "description": "claude-sonnet-4-6 / output_tokens",
                            "model": "claude-sonnet-4-6",
                            "token_type": "output_tokens",
                            "service_tier": "standard",
                            "context_window": "0-200k",
                            "workspace_id": null
                        },
                        {
                            "amount": "200.00",
                            "currency": "USD",
                            "cost_type": "tokens",
                            "description": "claude-haiku-4-5 / uncached_input_tokens",
                            "model": "claude-haiku-4-5",
                            "token_type": "uncached_input_tokens",
                            "service_tier": "standard",
                            "context_window": "0-200k",
                            "workspace_id": null
                        }
                    ]
                }
            ],
            "has_more": false,
            "next_page": null
        })
    }

    #[tokio::test]
    async fn parses_cost_report_one_page() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/organizations/cost_report"))
            .and(header("x-api-key", "sk-ant-admin01-FAKE"))
            .and(header("anthropic-version", "2023-06-01"))
            .and(query_param("bucket_width", "1d"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_response_body()))
            .mount(&server)
            .await;

        let client = AdminApiClient::with_base_url("sk-ant-admin01-FAKE", server.uri()).unwrap();
        let start = Utc.with_ymd_and_hms(2026, 4, 25, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 4, 26, 0, 0, 0).unwrap();
        let rows = client.fetch_cost_report(start, end).await.unwrap();

        assert_eq!(rows.len(), 2);
        let sonnet = rows
            .iter()
            .find(|r| r.model.as_deref() == Some("claude-sonnet-4-6"))
            .expect("sonnet row");
        assert!((sonnet.cost_usd.unwrap() - 12.345).abs() < 1e-6, "amount cents → dollars");
    }

    #[tokio::test]
    async fn unauthorized_surfaces_clear_hint() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/organizations/cost_report"))
            .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
            .mount(&server)
            .await;

        let client = AdminApiClient::with_base_url("sk-bad-key", server.uri()).unwrap();
        let start = Utc.with_ymd_and_hms(2026, 4, 25, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 4, 26, 0, 0, 0).unwrap();
        let err = client.fetch_cost_report(start, end).await.err().unwrap();
        let msg = format!("{err:#}");
        assert!(msg.contains("401"));
        assert!(msg.contains("admin"));
    }

    #[tokio::test]
    async fn paginates_when_has_more() {
        let server = MockServer::start().await;
        let page1 = serde_json::json!({
            "data": [{
                "starting_at": "2026-04-25T00:00:00Z",
                "ending_at": "2026-04-26T00:00:00Z",
                "results": [{
                    "amount": "100.00",
                    "currency": "USD",
                    "cost_type": "tokens",
                    "description": "page1",
                    "model": "claude-sonnet-4-6",
                    "token_type": "output_tokens"
                }]
            }],
            "has_more": true,
            "next_page": "tok-2"
        });
        let page2 = serde_json::json!({
            "data": [{
                "starting_at": "2026-04-26T00:00:00Z",
                "ending_at": "2026-04-27T00:00:00Z",
                "results": [{
                    "amount": "50.00",
                    "currency": "USD",
                    "cost_type": "tokens",
                    "description": "page2",
                    "model": "claude-haiku-4-5",
                    "token_type": "output_tokens"
                }]
            }],
            "has_more": false,
            "next_page": null
        });

        Mock::given(method("GET"))
            .and(path("/v1/organizations/cost_report"))
            .and(query_param("page", "tok-2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(page2))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/organizations/cost_report"))
            .respond_with(ResponseTemplate::new(200).set_body_json(page1))
            .mount(&server)
            .await;

        let client = AdminApiClient::with_base_url("sk-ant-admin01-FAKE", server.uri()).unwrap();
        let start = Utc.with_ymd_and_hms(2026, 4, 25, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 4, 27, 0, 0, 0).unwrap();
        let rows = client.fetch_cost_report(start, end).await.unwrap();
        assert_eq!(rows.len(), 2, "should follow pagination across both pages");
    }
}
