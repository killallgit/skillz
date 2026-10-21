//! Hook handlers in agent config files: finding them, grouping by tool, and provisioning them.
//! The text edits themselves live in `hook_text`.

mod discover;
mod group;
mod provision;

pub use discover::discover;
pub use group::hook_group;
pub use provision::provision;
