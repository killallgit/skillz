//! The lowest layer: what a coding tool's files and commands look like, and the actions that
//! change them. Nothing here may import `app`, `tui` or `cli` code; see `tests/architecture.rs`.

pub mod action;
pub mod agent_skills;
pub mod agents;
pub(crate) mod clone;
pub(crate) mod cost;
pub mod env;
pub(crate) mod fswalk;
pub(crate) mod hook_text;
pub(crate) mod hooks;
pub(crate) mod json_edit;
pub(crate) mod mcp;
pub mod model;
pub(crate) mod npx;
pub(crate) mod provision;
pub(crate) mod read_optional;
pub(crate) mod redact;
pub mod runner;
mod spec;
pub(crate) mod subagent_store;
pub(crate) mod subagents;
pub mod update;
pub mod usage;

pub use spec::Spec;
