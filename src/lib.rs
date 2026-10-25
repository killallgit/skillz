//! skillz: manage skills, plugins, marketplaces and hooks for coding agents.
//!
//! Layout: `harness::agents` read what is installed and describe how to remove it as
//! `Action`s; `harness::action` applies those actions; `cli` wires real dependencies to the flows.

pub mod app;
pub mod cli;
pub mod tui;
