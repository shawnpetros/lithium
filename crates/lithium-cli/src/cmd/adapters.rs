//! `lithium adapters` — list configured providers + last-poll status.

use anyhow::Result;
use chrono::Utc;
use lithium_core::config::Config;
use lithium_core::storage::Storage;
use lithium_core::types::{Provider, Source};

pub async fn run() -> Result<()> {
    let cfg = Config::load()?;
    let storage = Storage::open(cfg.db_path()?)?;

    println!("Adapters");
    println!();

    let anthropic_admin_configured = cfg
        .providers
        .anthropic
        .as_ref()
        .and_then(|a| a.admin_api_key.as_ref())
        .is_some();

    let anthropic_local_dir = cfg.claude_code_state_dir();
    let anthropic_local_configured = anthropic_local_dir.exists();

    let entries: Vec<(Provider, Source, bool, &'static str, String)> = vec![
        (
            Provider::Anthropic,
            Source::AdminApi,
            anthropic_admin_configured,
            "phase 1",
            "Anthropic Admin API (Cost Report)".into(),
        ),
        (
            Provider::Anthropic,
            Source::ClaudeCodeLocal,
            anthropic_local_configured,
            "phase 1",
            format!("Claude Code state at {}", anthropic_local_dir.display()),
        ),
        (
            Provider::OpenAI,
            Source::AdminApi,
            false,
            "phase 2",
            "(not implemented)".into(),
        ),
        (
            Provider::OpenRouter,
            Source::AdminApi,
            false,
            "phase 2",
            "(not implemented)".into(),
        ),
    ];

    for (provider, source, configured, phase, note) in entries {
        let status = storage.adapter_status(provider, source, configured)?;
        let icon = if !configured {
            "-"
        } else {
            match status.last_poll_status.as_deref() {
                Some("ok") => "✓",
                Some("error") => "✗",
                Some(_) => "·",
                None => "·",
            }
        };
        let label = format!("{provider} / {source}");
        let detail = if configured {
            match status.last_poll_at {
                Some(t) => {
                    let age = Utc::now() - t;
                    let s = status.last_poll_status.as_deref().unwrap_or("?");
                    let rows = status.last_poll_rows_inserted.unwrap_or(0);
                    format!(
                        "configured, last poll {} ({s}, {rows} rows)",
                        humanize_age(age)
                    )
                }
                None => "configured, never polled".to_string(),
            }
        } else {
            format!("not configured ({phase})")
        };
        println!("  {icon} {:<40} {detail}", label);
        println!("       {note}");
    }
    Ok(())
}

fn humanize_age(d: chrono::Duration) -> String {
    let s = d.num_seconds();
    if s < 60 {
        format!("{s}s ago")
    } else if s < 3600 {
        format!("{}m ago", s / 60)
    } else if s < 86400 {
        format!("{}h ago", s / 3600)
    } else {
        format!("{}d ago", s / 86400)
    }
}
