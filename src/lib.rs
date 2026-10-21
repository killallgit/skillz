//! skillz: manage skills, plugins, marketplaces and hooks for coding agents.
//!
//! Layout: `agents/*` discover what is installed (pure reads) and describe how to remove it as
//! `Action`s; `action` applies those actions; `cli` wires real dependencies to the flows.

pub mod action;
pub mod agents;
pub mod apply;
pub(crate) mod atomic;
pub mod cli;
pub(crate) mod clone;
pub(crate) mod cost;
pub mod env;
pub(crate) mod fswalk;
pub(crate) mod hook_text;
pub(crate) mod hooks;
pub(crate) mod info;
pub mod install;
pub(crate) mod json_edit;
pub mod library;
pub(crate) mod matcher;
pub(crate) mod mcp;
pub mod model;
pub(crate) mod npx_skills;
pub mod profile;
pub mod prompt;
pub(crate) mod provision;
pub mod prune;
pub(crate) mod read_optional;
pub(crate) mod redact;
pub(crate) mod remove;
pub mod review;
pub mod runner;
pub(crate) mod skills;
pub(crate) mod snapshot;
pub(crate) mod spinner;
pub mod store;
pub(crate) mod subagent_store;
pub(crate) mod subagents;
pub mod tree;
pub mod tui;
pub mod usage;
