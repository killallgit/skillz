//! The application layer: profiles, the library and store, comparing wanted with installed, and
//! the install, remove and prune flows. It depends on `skillz_harness` and on nothing above it;
//! see `tests/architecture.rs`.

pub mod app_paths;
pub mod apply;
pub(crate) mod info;
pub mod install;
pub mod json;
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
pub mod route;
pub mod rule;
pub(crate) mod snapshot;
pub(crate) mod spinner;
pub mod stale;
pub mod store;
pub mod sync;
pub mod tree;
pub mod workspace;
