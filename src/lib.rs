//! skillz: manage skills, plugins, marketplaces and hooks for coding agents.
//!
//! Layout: `agents/*` read what is installed and describe how to remove it as
//! `Action`s; `action` applies those actions; `cli` wires real dependencies to the flows.

pub mod action;
pub mod agents;
pub mod app_paths;
pub mod apply;
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
pub(crate) mod location;
pub(crate) mod matcher;
pub(crate) mod mcp;
pub mod merge;
pub mod model;
pub(crate) mod npx_skills;
pub mod package;
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
pub mod stale;
pub mod store;
pub(crate) mod subagent_store;
pub(crate) mod subagents;
pub mod sync;
pub mod tree;
pub mod tui;
pub mod update;
pub mod usage;
