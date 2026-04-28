//! TOML config loader at `~/.config/lithium/config.toml`.

use crate::error::{Error, Result};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Resolve the config root: `$XDG_CONFIG_HOME` if set, else `~/.config`.
///
/// We deliberately use XDG-style paths on all platforms, including macOS,
/// because that matches what other CLI tools do (gh, starship, helix, zellij)
/// and what users expect to type. The `directories::BaseDirs::config_dir()`
/// helper returns `~/Library/Application Support` on macOS, which is correct
/// per Apple's HIG but the wrong shape for a CLI tool.
fn xdg_config_home() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("XDG_CONFIG_HOME") {
        if !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    BaseDirs::new().map(|b| b.home_dir().join(".config"))
}

/// Resolve the data root: `$XDG_DATA_HOME` if set, else `~/.local/share`.
fn xdg_data_home() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("XDG_DATA_HOME") {
        if !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    BaseDirs::new().map(|b| b.home_dir().join(".local").join("share"))
}

/// Top-level config shape mapped to `~/.config/lithium/config.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub poll: PollConfig,
    #[serde(default)]
    pub providers: ProvidersConfig,
    #[serde(default)]
    pub fixed_costs: FixedCostsConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageConfig {
    /// Override database path. Default: `~/.local/share/lithium/usage.db`.
    pub db_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PollConfig {
    /// Default cadence in minutes. Phase 1 does not enforce this (no daemon yet).
    pub default_cadence_minutes: Option<u32>,
}

impl Default for PollConfig {
    fn default() -> Self {
        Self {
            default_cadence_minutes: Some(15),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvidersConfig {
    pub anthropic: Option<AnthropicConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnthropicConfig {
    /// Admin API key. Format: `sk-ant-admin01-...`. Required for Cost Report polling.
    pub admin_api_key: Option<String>,
    /// Override Claude Code state directory. Default: `~/.claude`.
    pub claude_code_state_dir: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FixedCostsConfig {
    /// User-declared monthly fixed subscription costs. Phase 1 displays these
    /// in `lithium month` so total = variable + fixed. Examples:
    ///   anthropic_max = 200.0   # $200/mo Claude Max plan
    ///   chatgpt_pro = 200.0
    ///
    /// Note: this struct intentionally does not use `deny_unknown_fields`
    /// because keys are arbitrary user-chosen labels, captured via flatten.
    #[serde(flatten)]
    pub items: std::collections::BTreeMap<String, f64>,
}

impl Config {
    /// Path to config file: `${XDG_CONFIG_HOME}/lithium/config.toml`,
    /// falling back to `~/.config/lithium/config.toml` on macOS and Linux.
    pub fn path() -> Result<PathBuf> {
        let base = xdg_config_home()
            .ok_or_else(|| Error::Config("cannot resolve home directory".into()))?;
        Ok(base.join("lithium").join("config.toml"))
    }

    /// Resolve the configured DB path, applying defaults and tilde expansion.
    pub fn db_path(&self) -> Result<PathBuf> {
        let raw = self
            .storage
            .db_path
            .clone()
            .unwrap_or_else(default_db_path_string);
        Ok(expand_tilde(&raw))
    }

    /// Resolve the configured Claude Code state dir.
    pub fn claude_code_state_dir(&self) -> PathBuf {
        let raw = self
            .providers
            .anthropic
            .as_ref()
            .and_then(|a| a.claude_code_state_dir.clone())
            .unwrap_or_else(|| "~/.claude".to_string());
        expand_tilde(&raw)
    }

    /// Load config from disk. Returns `Error::ConfigNotFound` if absent.
    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        if !path.exists() {
            return Err(Error::ConfigNotFound {
                path: path.display().to_string(),
            });
        }
        let raw = fs::read_to_string(&path)?;
        let cfg: Config = toml::from_str(&raw)?;
        Ok(cfg)
    }

    /// Write config to disk, creating parent directories as needed.
    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let serialized = toml::to_string_pretty(self)?;
        fs::write(&path, serialized)?;
        Ok(())
    }

    /// Render a fresh template with placeholders + comments. Used by `lithium config`
    /// when no config file exists yet.
    pub fn template() -> &'static str {
        TEMPLATE
    }
}

fn default_db_path_string() -> String {
    if let Some(base) = xdg_data_home() {
        base.join("lithium").join("usage.db").display().to_string()
    } else {
        "~/.local/share/lithium/usage.db".to_string()
    }
}

fn expand_tilde(raw: &str) -> PathBuf {
    if let Some(stripped) = raw.strip_prefix("~/") {
        if let Some(b) = BaseDirs::new() {
            return b.home_dir().join(stripped);
        }
    }
    PathBuf::from(raw)
}

const TEMPLATE: &str = r#"# lithium config
#
# Edit this file to wire up providers. Secrets stay on this machine only.
# lithium never logs admin keys; the value here is read on `lithium poll`
# and sent to the provider as the X-Api-Key header.

[storage]
# Optional. Default: ~/.local/share/lithium/usage.db (or XDG data dir on Linux).
# db_path = "/custom/path/usage.db"

[poll]
default_cadence_minutes = 15

[providers.anthropic]
# Generate an Admin API key at console.anthropic.com -> Settings -> Admin Keys.
# Admin keys start with `sk-ant-admin01-`. Distinct from regular API keys.
# admin_api_key = "sk-ant-admin01-PASTE-HERE"

# Optional. Default: ~/.claude
# claude_code_state_dir = "~/.claude"

# Phase 2 placeholders (not yet implemented):
# [providers.openai]
# admin_api_key = "..."
#
# [providers.openrouter]
# api_key = "..."

[fixed_costs]
# Monthly flat-rate subscriptions to add into `lithium month` totals.
# Keys are display labels, values are USD per month. Examples:
# anthropic_max = 200.0
# chatgpt_pro = 200.0
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn parses_template_clean() {
        let cfg: Config = toml::from_str(Config::template()).expect("template parses");
        assert_eq!(cfg.poll.default_cadence_minutes, Some(15));
    }

    #[test]
    fn round_trip_minimal() {
        let cfg = Config::default();
        let s = toml::to_string_pretty(&cfg).unwrap();
        let parsed: Config = toml::from_str(&s).unwrap();
        assert_eq!(parsed.poll.default_cadence_minutes, Some(15));
    }

    #[test]
    fn fixed_costs_round_trip() {
        let toml_str = r#"
            [fixed_costs]
            anthropic_max = 200.0
            chatgpt_pro = 200.0
        "#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.fixed_costs.items.get("anthropic_max"), Some(&200.0));
        assert_eq!(cfg.fixed_costs.items.get("chatgpt_pro"), Some(&200.0));
    }

    #[test]
    fn db_path_resolves_when_set() {
        let mut cfg = Config::default();
        cfg.storage.db_path = Some("/tmp/lithium-test.db".into());
        let p = cfg.db_path().unwrap();
        assert_eq!(p, PathBuf::from("/tmp/lithium-test.db"));
    }

    #[test]
    fn db_path_expands_tilde() {
        let mut cfg = Config::default();
        cfg.storage.db_path = Some("~/custom/usage.db".into());
        let p = cfg.db_path().unwrap();
        assert!(!p.to_string_lossy().contains('~'));
    }

    #[test]
    fn rejects_unknown_field() {
        let toml_str = r#"
            [storage]
            db_path = "/tmp/x.db"
            unknown_field = "boom"
        "#;
        let result: std::result::Result<Config, _> = toml::from_str(toml_str);
        assert!(result.is_err(), "unknown field should fail to parse");
    }

    #[test]
    fn save_creates_parent_dir() {
        let tmp = TempDir::new().unwrap();
        // shadow BaseDirs by setting XDG_CONFIG_HOME (the easy override)
        std::env::set_var("XDG_CONFIG_HOME", tmp.path());
        let cfg = Config::default();
        cfg.save().unwrap();
        let path = Config::path().unwrap();
        assert!(path.exists(), "config file should be written");
        let _reloaded = Config::load().unwrap();
        std::env::remove_var("XDG_CONFIG_HOME");
    }
}
