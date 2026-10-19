//! skillz: manage skills, plugins, marketplaces and hooks for coding agents.
//!
//! Layout: `agents/*` discover what is installed (pure reads) and describe how to remove it as
//! `Action`s; `action` applies those actions; `cli` wires real dependencies to the flows.

pub mod action;
pub mod agents;
pub mod cli;
pub mod env;
pub mod hooks;
pub mod install;
pub mod matcher;
pub mod model;
pub mod prompt;
pub mod runner;
pub mod skills;
pub mod tree;
pub mod tui;
