//! Subcommand dispatcher.

use anyhow::Result;
use clap::Subcommand;

mod adapters;
mod config_cmd;
mod doctor;
mod init;
mod month;
mod poll;
mod today;

#[derive(Subcommand)]
pub enum Command {
    /// Initialize the SQLite database (idempotent)
    Init,

    /// Open the config file in $EDITOR; create from template if missing
    Config,

    /// Poll all configured adapters and write usage data
    Poll {
        /// Restrict to a single provider (anthropic, openai, openrouter)
        #[arg(long)]
        provider: Option<String>,
    },

    /// Today's spend by provider/source with totals
    Today,

    /// Month-to-date spend with end-of-month projection
    Month,

    /// List configured adapters and last-poll status
    Adapters,

    /// Verify config + connectivity + DB health
    Doctor,
}

pub async fn dispatch(cmd: Command) -> Result<()> {
    match cmd {
        Command::Init => init::run().await,
        Command::Config => config_cmd::run().await,
        Command::Poll { provider } => poll::run(provider).await,
        Command::Today => today::run().await,
        Command::Month => month::run().await,
        Command::Adapters => adapters::run().await,
        Command::Doctor => doctor::run().await,
    }
}
