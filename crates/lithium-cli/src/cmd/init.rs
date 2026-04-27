//! `lithium init` — create the SQLite database and apply the schema.

use anyhow::Result;
use lithium_core::config::Config;
use lithium_core::storage::Storage;
use tracing::info;

pub async fn run() -> Result<()> {
    let cfg = match Config::load() {
        Ok(c) => c,
        Err(lithium_core::Error::ConfigNotFound { .. }) => {
            // OK to init without a config; we'll use defaults.
            Config::default()
        }
        Err(e) => return Err(e.into()),
    };
    let db_path = cfg.db_path()?;
    info!(db = %db_path.display(), "init");
    let storage = Storage::open(&db_path)?;
    println!("✓ Database initialized at {}", storage.path().display());
    println!("✓ Schema version: 1");
    Ok(())
}
