//! lithium-openai: OpenAI Costs API adapter
//!
//! Hits `GET https://api.openai.com/v1/organization/costs`. Requires an
//! OpenAI Admin API key (`sk-admin-...`), generated at
//! platform.openai.com -> Settings -> Organization -> Admin Keys. Distinct
//! from regular API keys (`sk-...`).
//!
//! Response shape: data[].results[] with `amount: { value, currency }`,
//! `line_item`, `project_id`. We group by line_item to surface what kind
//! of OpenAI service the spend is on (completions / training / etc).

use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use lithium_core::types::{Provider, Source, UsageRow};
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, info, warn};

const COSTS_PATH: &str = "/v1/organization/costs";

pub struct OpenAIClient {
    client: Client,
    base_url: String,
    admin_key: String,
}

impl OpenAIClient {
    pub fn new(admin_key: impl Into<String>) -> Result<Self> {
        Self::with_base_url(admin_key, "https://api.openai.com")
    }

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

    /// Fetch the costs report for the given UTC date range, paginating through.
    ///
    /// The OpenAI API uses Unix-seconds timestamps (`start_time`, `end_time`).
    pub async fn fetch_costs(
        &self,
        starting_at: DateTime<Utc>,
        ending_at: DateTime<Utc>,
    ) -> Result<Vec<UsageRow>> {
        let mut rows: Vec<UsageRow> = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let mut req = self
                .client
                .get(format!("{}{}", self.base_url, COSTS_PATH))
                .bearer_auth(&self.admin_key)
                .query(&[
                    ("start_time", starting_at.timestamp().to_string()),
                    ("end_time", ending_at.timestamp().to_string()),
                    ("bucket_width", "1d".into()),
                ])
                .query(&[("group_by[]", "line_item")]);

            if let Some(token) = &page_token {
                req = req.query(&[("page", token.as_str())]);
            }

            debug!(
                starting_at = %starting_at,
                ending_at = %ending_at,
                "GET openai costs"
            );
            let resp = req.send().await.context("openai costs request")?;
            let status = resp.status();
            let body_text = resp.text().await.context("read response body")?;

            if !status.is_success() {
                if status.as_u16() == 401 {
                    anyhow::bail!(
                        "401 Unauthorized from OpenAI Costs API. \
                         Hint: regenerate the admin key at platform.openai.com -> \
                         Settings -> Organization -> Admin Keys, then update \
                         ~/.config/lithium/config.toml"
                    );
                }
                if status.as_u16() == 429 {
                    anyhow::bail!(
                        "429 rate-limited by OpenAI Costs API; try again later. body={body_text}"
                    );
                }
                anyhow::bail!(
                    "openai costs returned HTTP {}: {}",
                    status.as_u16(),
                    truncate(&body_text, 500)
                );
            }

            let parsed: CostsResponse = serde_json::from_str(&body_text)
                .with_context(|| {
                    format!("parse openai costs body; raw={}", truncate(&body_text, 500))
                })?;

            let mut bucket_count = 0usize;
            for bucket in &parsed.data {
                bucket_count += 1;
                let bucket_start = unix_to_utc(bucket.start_time)?;
                let bucket_end = unix_to_utc(bucket.end_time)?;
                for result in &bucket.results {
                    if let Some(row) = cost_result_to_usage_row(bucket_start, bucket_end, result) {
                        rows.push(row);
                    }
                }
            }
            info!(
                buckets = bucket_count,
                page_more = parsed.has_more,
                "openai costs page processed"
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

    pub async fn fetch_month_to_date(&self, today_utc: DateTime<Utc>) -> Result<Vec<UsageRow>> {
        let month_start = NaiveDate::from_ymd_opt(today_utc.year(), today_utc.month(), 1)
            .context("invalid month start")?
            .and_hms_opt(0, 0, 0)
            .context("invalid hms")?;
        let starting_at = Utc.from_utc_datetime(&month_start);
        let ending_at = today_utc + chrono::Duration::days(1);
        self.fetch_costs(starting_at, ending_at).await
    }
}

#[derive(Debug, Deserialize)]
struct CostsResponse {
    data: Vec<CostsBucket>,
    has_more: bool,
    #[serde(default)]
    next_page: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CostsBucket {
    /// Unix seconds.
    start_time: i64,
    /// Unix seconds.
    end_time: i64,
    results: Vec<CostsResult>,
}

#[derive(Debug, Deserialize, Clone)]
struct CostsResult {
    #[serde(default)]
    amount: Option<Amount>,
    #[serde(default)]
    line_item: Option<String>,
    #[serde(default)]
    project_id: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct Amount {
    #[serde(default)]
    value: Option<f64>,
    #[serde(default)]
    currency: Option<String>,
}

fn cost_result_to_usage_row(
    period_start: DateTime<Utc>,
    period_end: DateTime<Utc>,
    result: &CostsResult,
) -> Option<UsageRow> {
    let amount = result.amount.as_ref()?;
    let cost_usd = amount.value?;
    if amount.currency.as_deref().map(str::to_lowercase).as_deref() != Some("usd")
        && amount.currency.is_some()
    {
        // Non-USD costs would need an FX conversion, deferred. Skip with a log.
        warn!(
            currency = %amount.currency.as_deref().unwrap_or("?"),
            "skipping non-USD cost row"
        );
        return None;
    }

    let raw = serde_json::json!({
        "amount": amount.value,
        "currency": amount.currency,
        "line_item": result.line_item,
        "project_id": result.project_id,
    });

    let model_label = result
        .line_item
        .clone()
        .map(|li| format!("openai.{li}"))
        .unwrap_or_else(|| "openai.unattributed".to_string());

    Some(
        UsageRow::new_api(Provider::OpenAI, Source::AdminApi, period_start, period_end, raw)
            .with_model(model_label)
            .with_cost_usd(cost_usd),
    )
}

fn unix_to_utc(secs: i64) -> Result<DateTime<Utc>> {
    Utc.timestamp_opt(secs, 0)
        .single()
        .ok_or_else(|| anyhow::anyhow!("invalid unix timestamp: {secs}"))
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…[truncated, {} chars]", &s[..max], s.len() - max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn sample_response_body() -> serde_json::Value {
        serde_json::json!({
            "object": "page",
            "data": [
                {
                    "object": "bucket",
                    "start_time": 1761609600,  // 2025-10-28 00:00 UTC
                    "end_time": 1761696000,    // 2025-10-29 00:00 UTC
                    "results": [
                        {
                            "object": "organization.costs.result",
                            "amount": {"value": 1.23, "currency": "usd"},
                            "line_item": "completions",
                            "project_id": null
                        },
                        {
                            "object": "organization.costs.result",
                            "amount": {"value": 0.45, "currency": "usd"},
                            "line_item": "embeddings",
                            "project_id": null
                        }
                    ]
                }
            ],
            "has_more": false,
            "next_page": null
        })
    }

    #[tokio::test]
    async fn parses_costs_one_page() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/organization/costs"))
            .and(header("authorization", "Bearer sk-admin-FAKE"))
            .and(query_param("bucket_width", "1d"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_response_body()))
            .mount(&server)
            .await;

        let client = OpenAIClient::with_base_url("sk-admin-FAKE", server.uri()).unwrap();
        let start = Utc.with_ymd_and_hms(2025, 10, 28, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2025, 10, 29, 0, 0, 0).unwrap();
        let rows = client.fetch_costs(start, end).await.unwrap();

        assert_eq!(rows.len(), 2);
        let completions = rows
            .iter()
            .find(|r| r.model.as_deref() == Some("openai.completions"))
            .expect("completions row");
        assert!((completions.cost_usd.unwrap() - 1.23).abs() < 1e-9);
    }

    #[tokio::test]
    async fn unauthorized_surfaces_clear_hint() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/organization/costs"))
            .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
            .mount(&server)
            .await;

        let client = OpenAIClient::with_base_url("sk-admin-BAD", server.uri()).unwrap();
        let start = Utc.with_ymd_and_hms(2025, 10, 28, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2025, 10, 29, 0, 0, 0).unwrap();
        let err = client.fetch_costs(start, end).await.err().unwrap();
        let msg = format!("{err:#}");
        assert!(msg.contains("401"));
        assert!(msg.contains("Admin Keys"));
    }

    #[tokio::test]
    async fn paginates_when_has_more() {
        let server = MockServer::start().await;
        let page1 = serde_json::json!({
            "object": "page",
            "data": [{
                "object": "bucket",
                "start_time": 1761609600,
                "end_time": 1761696000,
                "results": [{"amount": {"value": 1.0, "currency": "usd"}, "line_item": "p1"}]
            }],
            "has_more": true,
            "next_page": "tok-2"
        });
        let page2 = serde_json::json!({
            "object": "page",
            "data": [{
                "object": "bucket",
                "start_time": 1761696000,
                "end_time": 1761782400,
                "results": [{"amount": {"value": 2.0, "currency": "usd"}, "line_item": "p2"}]
            }],
            "has_more": false,
            "next_page": null
        });

        Mock::given(method("GET"))
            .and(path("/v1/organization/costs"))
            .and(query_param("page", "tok-2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(page2))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/organization/costs"))
            .respond_with(ResponseTemplate::new(200).set_body_json(page1))
            .mount(&server)
            .await;

        let client = OpenAIClient::with_base_url("sk-admin-FAKE", server.uri()).unwrap();
        let start = Utc.with_ymd_and_hms(2025, 10, 28, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2025, 10, 30, 0, 0, 0).unwrap();
        let rows = client.fetch_costs(start, end).await.unwrap();
        assert_eq!(rows.len(), 2, "should follow pagination across both pages");
    }
}
