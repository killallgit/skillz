//! skillz: manage skills, plugins, marketplaces and hooks for coding agents.
//!
//! Layout: the `skillz_harness` crate reads what is installed and describes how to change it as
//! `Action`s and applies them; `app` decides what to do; `tui` draws; `cli` wires real
//! dependencies to the flows.

pub mod app;
pub mod cli;
pub mod tui;
