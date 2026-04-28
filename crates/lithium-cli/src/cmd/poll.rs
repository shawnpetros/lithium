//! `lithium poll` — run all configured adapters and write usage data.

use anyhow::Result;
use chrono::Utc;
use lithium_anthropic::{AdminApiClient, ClaudeCodeReader};
use lithium_core::config::Config;
use lithium_core::storage::Storage;
use lithium_core::types::{Provider, Source};
use lithium_openai::OpenAIClient;
use lithium_openrouter::OpenRouterClient;
use tracing::{error, info, warn};

pub async fn run(provider_filter: Option<String>) -> Result<()> {
    let cfg = Config::load()?;
    let db_path = cfg.db_path()?;
    let storage = Storage::open(&db_path)?;

    let want = |p: &str| match &provider_filter {
        Some(filter) => filter == p,
        None => true,
    };

    if want("anthropic") {
        poll_anthropic_admin(&cfg, &storage).await;
        poll_claude_code_local(&cfg, &storage).await;
    }
    if want("openai") {
        poll_openai_costs(&cfg, &storage).await;
    }
    if want("openrouter") {
        poll_openrouter(&cfg, &storage).await;
    }

    if let Some(p) = &provider_filter {
        if !["anthropic", "openai", "openrouter"].contains(&p.as_str()) {
            warn!("unknown provider filter: {p}");
        }
    }

    Ok(())
}

async fn poll_anthropic_admin(cfg: &Config, storage: &Storage) {
    let admin_key = cfg
        .providers
        .anthropic
        .as_ref()
        .and_then(|a| a.admin_api_key.clone());

    let started_at = Utc::now();
    let Some(key) = admin_key else {
        let msg = "anthropic admin_api_key not set in config";
        info!(msg);
        println!("- anthropic / admin_api    not configured (set admin_api_key in ~/.config/lithium/config.toml)");
        let _ = storage.record_poll(
            Provider::Anthropic,
            Source::AdminApi,
            started_at,
            Utc::now(),
            "skipped",
            0,
            Some(msg),
        );
        return;
    };

    info!("polling anthropic admin api");
    let client = match AdminApiClient::new(key) {
        Ok(c) => c,
        Err(e) => {
            error!("admin api client init failed: {e:#}");
            println!("✗ anthropic / admin_api    error: {}", first_line(&format!("{e:#}")));
            let _ = storage.record_poll(
                Provider::Anthropic,
                Source::AdminApi,
                started_at,
                Utc::now(),
                "error",
                0,
                Some(&format!("{e:#}")),
            );
            return;
        }
    };

    match client.fetch_month_to_date(Utc::now()).await {
        Ok(rows) => {
            let mut written = 0u64;
            for row in &rows {
                if let Err(e) = storage.upsert_usage(row) {
                    warn!("upsert failed: {e:#}");
                } else {
                    written += 1;
                }
            }
            let _ = storage.record_poll(
                Provider::Anthropic,
                Source::AdminApi,
                started_at,
                Utc::now(),
                "ok",
                written,
                None,
            );
            println!(
                "✓ anthropic / admin_api          {written} rows inserted ({} unique models)",
                count_unique_models(&rows)
            );
        }
        Err(e) => {
            error!("cost report fetch failed: {e:#}");
            println!("✗ anthropic / admin_api    error: {}", first_line(&format!("{e:#}")));
            let _ = storage.record_poll(
                Provider::Anthropic,
                Source::AdminApi,
                started_at,
                Utc::now(),
                "error",
                0,
                Some(&format!("{e:#}")),
            );
        }
    }
}

async fn poll_claude_code_local(cfg: &Config, storage: &Storage) {
    let started_at = Utc::now();
    let dir = cfg.claude_code_state_dir();
    let reader = ClaudeCodeReader::new(&dir);

    if !reader.is_available() {
        info!("claude code state dir missing");
        println!(
            "- anthropic / claude_code_local   not available ({} not found)",
            dir.display()
        );
        let _ = storage.record_poll(
            Provider::Anthropic,
            Source::ClaudeCodeLocal,
            started_at,
            Utc::now(),
            "skipped",
            0,
            Some("state dir missing"),
        );
        return;
    }

    match reader.read_daily_token_rows() {
        Ok(rows) => {
            let mut written = 0u64;
            for row in &rows {
                if let Err(e) = storage.upsert_usage(row) {
                    warn!("upsert failed: {e:#}");
                } else {
                    written += 1;
                }
            }
            let _ = storage.record_poll(
                Provider::Anthropic,
                Source::ClaudeCodeLocal,
                started_at,
                Utc::now(),
                "ok",
                written,
                None,
            );
            println!("✓ anthropic / claude_code_local   {written} rows inserted (per-day per-model token volume)");
        }
        Err(e) => {
            error!("claude code read failed: {e:#}");
            println!("✗ anthropic / claude_code_local   error: {}", first_line(&format!("{e:#}")));
            let _ = storage.record_poll(
                Provider::Anthropic,
                Source::ClaudeCodeLocal,
                started_at,
                Utc::now(),
                "error",
                0,
                Some(&format!("{e:#}")),
            );
        }
    }
}

async fn poll_openai_costs(cfg: &Config, storage: &Storage) {
    let started_at = Utc::now();
    let admin_key = cfg
        .providers
        .openai
        .as_ref()
        .and_then(|o| o.admin_api_key.clone());

    let Some(key) = admin_key else {
        let msg = "openai admin_api_key not set in config";
        info!(msg);
        println!("- openai / admin_api       not configured (set admin_api_key in ~/.config/lithium/config.toml)");
        let _ = storage.record_poll(
            Provider::OpenAI,
            Source::AdminApi,
            started_at,
            Utc::now(),
            "skipped",
            0,
            Some(msg),
        );
        return;
    };

    info!("polling openai costs");
    let client = match OpenAIClient::new(key) {
        Ok(c) => c,
        Err(e) => {
            error!("openai client init failed: {e:#}");
            println!("✗ openai / admin_api       error: {}", first_line(&format!("{e:#}")));
            let _ = storage.record_poll(
                Provider::OpenAI,
                Source::AdminApi,
                started_at,
                Utc::now(),
                "error",
                0,
                Some(&format!("{e:#}")),
            );
            return;
        }
    };

    match client.fetch_month_to_date(Utc::now()).await {
        Ok(rows) => {
            let mut written = 0u64;
            for row in &rows {
                if let Err(e) = storage.upsert_usage(row) {
                    warn!("upsert failed: {e:#}");
                } else {
                    written += 1;
                }
            }
            let _ = storage.record_poll(
                Provider::OpenAI,
                Source::AdminApi,
                started_at,
                Utc::now(),
                "ok",
                written,
                None,
            );
            println!(
                "✓ openai / admin_api             {written} rows inserted ({} line items)",
                count_unique_models(&rows)
            );
        }
        Err(e) => {
            error!("openai costs fetch failed: {e:#}");
            println!("✗ openai / admin_api       error: {}", first_line(&format!("{e:#}")));
            let _ = storage.record_poll(
                Provider::OpenAI,
                Source::AdminApi,
                started_at,
                Utc::now(),
                "error",
                0,
                Some(&format!("{e:#}")),
            );
        }
    }
}

async fn poll_openrouter(cfg: &Config, storage: &Storage) {
    let started_at = Utc::now();
    let api_key = cfg
        .providers
        .openrouter
        .as_ref()
        .and_then(|o| o.api_key.clone());

    let Some(key) = api_key else {
        let msg = "openrouter api_key not set in config";
        info!(msg);
        println!("- openrouter / admin_api   not configured (set api_key in ~/.config/lithium/config.toml)");
        let _ = storage.record_poll(
            Provider::OpenRouter,
            Source::AdminApi,
            started_at,
            Utc::now(),
            "skipped",
            0,
            Some(msg),
        );
        return;
    };

    info!("polling openrouter");
    let client = match OpenRouterClient::new(key) {
        Ok(c) => c,
        Err(e) => {
            error!("openrouter client init failed: {e:#}");
            println!("✗ openrouter / admin_api   error: {}", first_line(&format!("{e:#}")));
            let _ = storage.record_poll(
                Provider::OpenRouter,
                Source::AdminApi,
                started_at,
                Utc::now(),
                "error",
                0,
                Some(&format!("{e:#}")),
            );
            return;
        }
    };

    match client.fetch_usage().await {
        Ok(rows) => {
            let mut written = 0u64;
            for row in &rows {
                if let Err(e) = storage.upsert_usage(row) {
                    warn!("upsert failed: {e:#}");
                } else {
                    written += 1;
                }
            }
            let _ = storage.record_poll(
                Provider::OpenRouter,
                Source::AdminApi,
                started_at,
                Utc::now(),
                "ok",
                written,
                None,
            );
            println!("✓ openrouter / admin_api         {written} rows inserted (today's usage_daily)");
        }
        Err(e) => {
            error!("openrouter fetch failed: {e:#}");
            println!("✗ openrouter / admin_api   error: {}", first_line(&format!("{e:#}")));
            let _ = storage.record_poll(
                Provider::OpenRouter,
                Source::AdminApi,
                started_at,
                Utc::now(),
                "error",
                0,
                Some(&format!("{e:#}")),
            );
        }
    }
}

fn count_unique_models(rows: &[lithium_core::types::UsageRow]) -> usize {
    let mut set = std::collections::BTreeSet::new();
    for r in rows {
        if let Some(m) = &r.model {
            set.insert(m.clone());
        }
    }
    set.len()
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").to_string()
}
