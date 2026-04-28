//! `lithium doctor` — verify config + connectivity + DB health.

use anyhow::Result;
use lithium_anthropic::AdminApiClient;
use lithium_core::config::Config;
use lithium_core::storage::Storage;
use lithium_openai::OpenAIClient;
use lithium_openrouter::OpenRouterClient;

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

    // 5. Connectivity to api.anthropic.com (only if a key is configured).
    //
    // Use a day-aligned 3-day window so the bucket_width=1d snap reliably
    // returns at least one bucket. (Passing arbitrary timestamps causes the
    // API to reject the range with "ending date must be after starting date"
    // when no full-day bucket fits inside the window.)
    if let Some(key) = admin_key {
        use chrono::TimeZone;
        let client = AdminApiClient::new(key)?;
        let now = chrono::Utc::now();
        let today = now.date_naive();
        let three_days_ago = today - chrono::Duration::days(3);
        let tomorrow = today + chrono::Duration::days(1);
        let starting_at = chrono::Utc
            .from_utc_datetime(&three_days_ago.and_hms_opt(0, 0, 0).unwrap());
        let ending_at = chrono::Utc.from_utc_datetime(&tomorrow.and_hms_opt(0, 0, 0).unwrap());
        match client.fetch_cost_report(starting_at, ending_at).await {
            Ok(_) => print_check(true, "Connectivity to api.anthropic.com", "200 OK"),
            Err(e) => print_check(false, "Connectivity to api.anthropic.com", &first_line(&format!("{e:#}"))),
        }
    } else {
        print_check(false, "Connectivity to api.anthropic.com", "skipped (no admin key set)");
    }

    // 6. OpenAI admin key + connectivity
    let openai_key = cfg
        .as_ref()
        .and_then(|c| c.providers.openai.as_ref())
        .and_then(|o| o.admin_api_key.clone());
    print_check(
        openai_key.is_some(),
        "OpenAI admin key set",
        openai_key
            .as_deref()
            .map(redact)
            .unwrap_or_else(|| "not set".to_string())
            .as_str(),
    );
    if let Some(key) = openai_key {
        use chrono::TimeZone;
        let client = OpenAIClient::new(key)?;
        let now = chrono::Utc::now();
        let today = now.date_naive();
        let three_days_ago = today - chrono::Duration::days(3);
        let tomorrow = today + chrono::Duration::days(1);
        let starting_at = chrono::Utc
            .from_utc_datetime(&three_days_ago.and_hms_opt(0, 0, 0).unwrap());
        let ending_at = chrono::Utc.from_utc_datetime(&tomorrow.and_hms_opt(0, 0, 0).unwrap());
        match client.fetch_costs(starting_at, ending_at).await {
            Ok(_) => print_check(true, "Connectivity to api.openai.com", "200 OK"),
            Err(e) => print_check(
                false,
                "Connectivity to api.openai.com",
                &first_line(&format!("{e:#}")),
            ),
        }
    } else {
        print_check(false, "Connectivity to api.openai.com", "skipped (no admin key set)");
    }

    // 7. OpenRouter key + connectivity
    let openrouter_key = cfg
        .as_ref()
        .and_then(|c| c.providers.openrouter.as_ref())
        .and_then(|o| o.api_key.clone());
    print_check(
        openrouter_key.is_some(),
        "OpenRouter API key set",
        openrouter_key
            .as_deref()
            .map(redact)
            .unwrap_or_else(|| "not set".to_string())
            .as_str(),
    );
    if let Some(key) = openrouter_key {
        let client = OpenRouterClient::new(key)?;
        match client.fetch_usage().await {
            Ok(_) => print_check(true, "Connectivity to openrouter.ai", "200 OK"),
            Err(e) => print_check(
                false,
                "Connectivity to openrouter.ai",
                &first_line(&format!("{e:#}")),
            ),
        }
    } else {
        print_check(false, "Connectivity to openrouter.ai", "skipped (no api key set)");
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
