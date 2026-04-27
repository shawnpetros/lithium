//! lithium-anthropic: Anthropic adapter
//!
//! Two sources behind one provider:
//! - `admin_api`: Anthropic Cost Report API (`/v1/organizations/cost_report`)
//! - `claude_code_local`: `~/.claude/stats-cache.json` parser
//!
//! See `docs/SPEC-PHASE-1.md` at the repo root for the contract.

pub mod admin_api;
pub mod claude_code;

pub use admin_api::AdminApiClient;
pub use claude_code::ClaudeCodeReader;
