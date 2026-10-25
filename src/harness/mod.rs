//! The lowest layer: what a coding tool's files and commands look like, and the actions that
//! change them. Nothing here may import `app`, `tui` or `cli` code; see `tests/architecture.rs`.

pub mod action;
pub mod agent_skills;
pub(crate) mod clone;
pub(crate) mod fswalk;
pub(crate) mod json_edit;
pub(crate) mod read_optional;
pub(crate) mod redact;
pub mod runner;
mod spec;

pub use spec::Spec;
