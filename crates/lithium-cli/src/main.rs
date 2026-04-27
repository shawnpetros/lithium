//! lithium CLI binary entry point.
//!
//! Subcommands: init, config, poll, today, month, adapters, doctor.
//! See `docs/SPEC-PHASE-1.md` for the spec.

use anyhow::Result;
use clap::Parser;

mod cmd;

/// lithium - Mood stabilizer for your AI bill.
///
/// Cross-provider LLM-spend aggregator. One number, every provider, no spreadsheet.
#[derive(Parser)]
#[command(name = "lithium", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: cmd::Command,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();
    cmd::dispatch(cli.command).await
}

fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("lithium=info"));

    fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr) // diagnostics to stderr; user output via println! to stdout
        .with_target(false)
        .compact()
        .init();
}
