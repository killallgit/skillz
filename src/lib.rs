//! skillz: manage skills, plugins, marketplaces and hooks for coding agents.
//!
//! Layout: `harness::agents` read what is installed and describe how to remove it as
//! `Action`s; `harness::action` applies those actions; `cli` wires real dependencies to the flows.

pub mod app_paths;
pub mod apply;
pub mod cli;
pub(crate) mod info;
pub mod install;
pub mod library;
pub(crate) mod location;
pub(crate) mod matcher;
pub mod merge;
pub mod package;
pub mod profile;
pub mod prompt;
pub mod prune;
pub(crate) mod remove;
pub mod review;
pub(crate) mod snapshot;
pub(crate) mod spinner;
pub mod stale;
pub mod store;
pub mod sync;
pub mod tree;
pub mod tui;
pub mod workspace;
