//! `lithium month` — month-to-date with end-of-month projection.

use anyhow::Result;
use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use lithium_core::config::Config;
use lithium_core::projection::{MonthProjectionInput, days_elapsed, days_in_month, project_month};
use lithium_core::storage::Storage;

pub async fn run() -> Result<()> {
    let cfg = Config::load()?;
    let storage = Storage::open(cfg.db_path()?)?;

    let now = Utc::now();
    let today = now.date_naive();
    let (year, month) = (today.year(), today.month());

    let month_start = NaiveDate::from_ymd_opt(year, month, 1).expect("valid first-of-month");
    let next_month = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap()
    };
    let start_utc = Utc.from_utc_datetime(&month_start.and_hms_opt(0, 0, 0).unwrap());
    let end_utc = Utc.from_utc_datetime(&next_month.and_hms_opt(0, 0, 0).unwrap());

    let variable_to_date = storage.total_variable_cost(start_utc, end_utc)?;
    let fixed_total: f64 = cfg.fixed_costs.items.values().sum();
    let elapsed = days_elapsed(year, month, today);
    let total_days = days_in_month(year, month);

    let proj = project_month(MonthProjectionInput {
        variable_to_date,
        fixed_total,
        days_elapsed: elapsed,
        days_in_month: total_days,
    });

    println!(
        "lithium - {} (day {} of {})",
        format_month(year, month),
        elapsed,
        total_days
    );
    println!();

    // Per-model breakdown of variable cost.
    let rows = storage.cost_by_model_in_window(start_utc, end_utc)?;
    let mut by_provider: std::collections::BTreeMap<String, Vec<&lithium_core::storage::DailyCostRow>> =
        std::collections::BTreeMap::new();
    for r in &rows {
        by_provider.entry(r.provider.clone()).or_default().push(r);
    }
    for (provider, prov_rows) in &by_provider {
        println!("{}", capitalize(provider));
        let mut subtotal = 0.0;
        for row in prov_rows {
            let label = format!("{} / {}", row.source, row.model);
            println!("  {:<40} ${:>9.2}", label, row.cost_usd);
            subtotal += row.cost_usd;
        }
        if !prov_rows.is_empty() {
            println!(
                "  {:<40} ${:>9.2}",
                "(provider subtotal)", subtotal
            );
        }
        println!();
    }

    // Fixed costs.
    if !cfg.fixed_costs.items.is_empty() {
        println!("Fixed monthly costs (declared in config)");
        for (k, v) in &cfg.fixed_costs.items {
            println!("  {:<40} ${:>9.2}", k, v);
        }
        println!("  {:<40} ${:>9.2}", "(fixed subtotal)", fixed_total);
        println!();
    }

    println!("Variable to date:           ${:>9.2}", normalize_zero(proj.variable_to_date));
    println!("Daily avg (variable):       ${:>9.2}", normalize_zero(proj.variable_daily_avg));
    println!("Fixed total:                ${:>9.2}", normalize_zero(proj.fixed_total));
    println!("Projected variable EOM:     ${:>9.2}", normalize_zero(proj.projected_variable_eom));
    println!("---");
    println!("Projected total EOM:        ${:>9.2}", normalize_zero(proj.projected_total_eom));

    Ok(())
}

/// Replace IEEE-754 negative zero with positive zero for display.
/// (`-0.0 == 0.0` is true in Rust but `format!("{:.2}", -0.0)` prints `-0.00`.)
fn normalize_zero(v: f64) -> f64 {
    if v == 0.0 { 0.0 } else { v }
}

fn format_month(year: i32, month: u32) -> String {
    const NAMES: [&str; 12] = [
        "January", "February", "March", "April", "May", "June", "July", "August", "September",
        "October", "November", "December",
    ];
    let name = NAMES.get((month as usize).saturating_sub(1)).copied().unwrap_or("?");
    format!("{name} {year}")
}

fn capitalize(s: &str) -> String {
    match s {
        "anthropic" => "Anthropic".to_string(),
        "openai" => "OpenAI".to_string(),
        "openrouter" => "OpenRouter".to_string(),
        other => {
            let mut c = other.chars();
            match c.next() {
                Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        }
    }
}
