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

    let editor_raw = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vim".to_string());

    // $EDITOR may include args (e.g., "nvim -f", "code -w"). Split into binary + args.
    let mut parts = editor_raw.split_whitespace();
    let bin = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("$EDITOR is empty"))?;
    let extra_args: Vec<&str> = parts.collect();

    let status = Command::new(bin)
        .args(&extra_args)
        .arg(&path)
        .status()
        .with_context(|| format!("launch editor `{editor_raw}`"))?;

    if !status.success() {
        anyhow::bail!("editor `{editor_raw}` exited with {status}");
    }
    Ok(())
}
