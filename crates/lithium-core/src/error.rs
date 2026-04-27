//! Crate-level errors.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("config: {0}")]
    Config(String),

    #[error("config file not found at {path}; run `lithium config` to create one")]
    ConfigNotFound { path: String },

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("toml parse: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("toml serialize: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("storage: {0}")]
    Storage(String),
}

pub type Result<T> = std::result::Result<T, Error>;
