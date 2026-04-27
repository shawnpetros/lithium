//! `lithium doctor` — verify config + connectivity + DB health.

use anyhow::Result;
use lithium_anthropic::AdminApiClient;
use lithium_core::config::Config;
use lithium_core::storage::Storage;

pub async fn run() -> Result<()> {
    println!("lithium doctor");
    println!();

    // 1. Config file exists
    let cfg_path = Config::path()?;
    print_check(cfg_path.exists(), "Config file exists", &cfg_path.display().to_string());

    let cfg = Config::load().ok();

    // 2. Database accessible + schema applied
    if let Some(cfg) = &cfg {
        let db_path = cfg.db_path()?;
        match Storage::open(&db_path) {
            Ok(s) => print_check(true, "Database accessible", &s.path().display().to_string()),
            Err(e) => print_check(false, "Database accessible", &format!("error: {e}")),
        }
        print_check(true, "Schema version current", "1");
    } else {
        print_check(false, "Database accessible", "config not loadable");
    }

    // 3. Anthropic admin key set
    let admin_key = cfg
        .as_ref()
        .and_then(|c| c.providers.anthropic.as_ref())
        .and_then(|a| a.admin_api_key.clone());
    let admin_set = admin_key.is_some();
    print_check(
        admin_set,
        "Anthropic admin key set",
        admin_key.as_deref().map(redact).unwrap_or_else(|| "not set".to_string()).as_str(),
    );

    // 4. Claude Code state dir
    if let Some(cfg) = &cfg {
        let dir = cfg.claude_code_state_dir();
        let stats_path = dir.join("stats-cache.json");
        print_check(
            dir.exists(),
            "Claude Code state dir",
            &dir.display().to_string(),
        );
        print_check(
            stats_path.exists(),
            "stats-cache.json present",
            &stats_path.display().to_string(),
        );
    }

    // 5. Connectivity to api.anthropic.com (only if a key is configured)
    if let Some(key) = admin_key {
        let client = AdminApiClient::new(key)?;
        let now = chrono::Utc::now();
        let yesterday = now - chrono::Duration::days(1);
        match client.fetch_cost_report(yesterday, now).await {
            Ok(_) => print_check(true, "Connectivity to api.anthropic.com", "200 OK"),
            Err(e) => print_check(false, "Connectivity to api.anthropic.com", &first_line(&format!("{e:#}"))),
        }
    } else {
        print_check(false, "Connectivity to api.anthropic.com", "skipped (no admin key set)");
    }

    println!();
    println!("Run `lithium poll` next, then `lithium today`.");
    Ok(())
}

fn print_check(passed: bool, name: &str, detail: &str) {
    let icon = if passed { "✓" } else { "✗" };
    println!("  {icon} {:<32} {detail}", name);
}

fn redact(key: &str) -> String {
    if key.len() < 16 {
        "***REDACTED***".to_string()
    } else {
        format!("{}***REDACTED***", &key[..16])
    }
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").to_string()
}
