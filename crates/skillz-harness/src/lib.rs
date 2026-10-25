//! The lowest layer: what a coding tool's files and commands look like, and the actions that
//! change them. This crate depends on nothing of skillz's, so cargo itself keeps `app`, `tui` and `cli` out.

pub mod action;
pub mod agent_skills;
pub mod agents;
pub mod clone;
pub mod cost;
pub mod env;
pub mod fswalk;
pub mod hook_text;
pub mod hooks;
pub mod json_edit;
pub mod mcp;
pub mod model;
pub mod npx;
pub mod provision;
pub mod read_optional;
pub mod redact;
pub mod runner;
mod spec;
pub mod subagent_store;
pub mod subagents;
pub mod test_support;
pub mod update;
pub mod usage;

pub use spec::Spec;
