//! `lithium today` — today's spend by provider/source with totals.

use anyhow::Result;
use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use lithium_core::config::Config;
use lithium_core::storage::Storage;
use lithium_core::types::{Provider, Source};

pub async fn run() -> Result<()> {
    let cfg = Config::load()?;
    let storage = Storage::open(cfg.db_path()?)?;

    let now = Utc::now();
    let today = now.date_naive();
    let start = day_start(today);
    let end = day_end(today);

    let mut rows = storage.cost_by_model_in_window(start, end)?;
    rows.sort_by(|a, b| {
        b.cost_usd
            .partial_cmp(&a.cost_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    println!("lithium - {}", today);
    println!();

    // Group by provider for display.
    let mut by_provider: std::collections::BTreeMap<String, Vec<&lithium_core::storage::DailyCostRow>> =
        std::collections::BTreeMap::new();
    for r in &rows {
        by_provider.entry(r.provider.clone()).or_default().push(r);
    }

    if rows.is_empty() {
        let last_admin = storage.last_poll(Provider::Anthropic, Source::AdminApi)?;
        let last_local = storage.last_poll(Provider::Anthropic, Source::ClaudeCodeLocal)?;
        if last_admin.is_none() && last_local.is_none() {
            println!("(no data yet, run `lithium poll` to populate)");
            return Ok(());
        }
        // We have polled before, but today's bucket is empty. Most common cause:
        // the Anthropic Cost Report API has up to 24h reporting lag, so today's
        // bucket isn't populated yet. Surface this explicitly so the user doesn't
        // assume the tool is broken.
        println!("(no rows for today yet)");
        println!("The Anthropic Cost Report API typically has up to 24h reporting lag.");
        println!("Try `lithium month` to see month-to-date, or repoll later in the day.");
        println!();
    }

    let mut total_today = 0.0;
    for (provider, prov_rows) in &by_provider {
        println!("{}", capitalize(provider));
        let mut provider_subtotal = 0.0;
        for row in prov_rows {
            let label = format!("{} / {}", row.source, row.model);
            println!(
                "  {:<40} ${:>8.2}",
                label,
                row.cost_usd
            );
            provider_subtotal += row.cost_usd;
        }
        if !prov_rows.is_empty() {
            println!("  {:<40} ${:>8.2}", "(provider subtotal)", provider_subtotal);
        }
        total_today += provider_subtotal;
        println!();
    }

    // Claude Code session/weekly limits, if any. (Phase 1: stats-cache.json doesn't
    // include pct, so this typically prints nothing. Wired for future Phase 1.5.)
    if let Some(limits) = storage.claude_code_limits_on(today)? {
        if let Some(p) = limits.session_pct {
            println!(
                "Claude Code session   {:.0}%{}",
                p * 100.0,
                limits
                    .session_resets_at
                    .map(|t| format!(" (resets at {t})"))
                    .unwrap_or_default()
            );
        }
        if let Some(p) = limits.weekly_pct {
            println!(
                "Claude Code weekly    {:.0}%{}",
                p * 100.0,
                limits
                    .weekly_resets_at
                    .map(|t| format!(" (resets at {t})"))
                    .unwrap_or_default()
            );
        }
        println!();
    }

    println!("Total today: ${:.2}", total_today);

    let last = storage
        .last_poll(Provider::Anthropic, Source::AdminApi)?
        .or(storage.last_poll(Provider::Anthropic, Source::ClaudeCodeLocal)?);
    if let Some(last) = last {
        if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(&last.started_at) {
            let age = now - parsed.with_timezone(&Utc);
            println!();
            println!("Last polled: {}", humanize_age(age));
        }
    }

    Ok(())
}

fn day_start(d: NaiveDate) -> chrono::DateTime<chrono::Utc> {
    Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap())
}

fn day_end(d: NaiveDate) -> chrono::DateTime<chrono::Utc> {
    let next = d.succ_opt().unwrap();
    Utc.from_utc_datetime(&next.and_hms_opt(0, 0, 0).unwrap())
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
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

// Suppress unused warning until projection helpers are wired.
#[allow(dead_code)]
fn _datelike_anchor(_d: chrono::DateTime<Utc>) -> i32 {
    Utc::now().year()
}
