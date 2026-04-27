//! lithium-core: types, SQLite storage, config, projection logic
//!
//! See `docs/SPEC-PHASE-1.md` at the repo root for the design contract.

pub mod config;
pub mod error;
pub mod projection;
pub mod storage;
pub mod types;

pub use error::{Error, Result};
pub use types::{Provider, Source, UsageRow};
