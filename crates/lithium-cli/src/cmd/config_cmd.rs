//! `lithium config` — open the config file in $EDITOR; create from template if missing.

use anyhow::{Context, Result};
use lithium_core::config::Config;
use std::process::Command;
use tracing::info;

pub async fn run() -> Result<()> {
    let path = Config::path()?;
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create config dir {}", parent.display()))?;
        }
        std::fs::write(&path, Config::template())
            .with_context(|| format!("write config template to {}", path.display()))?;
        println!("✓ Wrote config template at {}", path.display());
        info!(path = %path.display(), "config template written");
    } else {
        println!("Editing {}", path.display());
    }

    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vim".to_string());

    let status = Command::new(&editor)
        .arg(&path)
        .status()
        .with_context(|| format!("launch editor `{editor}`"))?;

    if !status.success() {
        anyhow::bail!("editor `{editor}` exited with {status}");
    }
    Ok(())
}
