//! Hook handlers in agent config files: finding them, grouping by tool, and provisioning them.
//! The text edits themselves live in `hook_text`.

mod discover;
mod group;
mod plugin;
mod provision;

pub use discover::{UNOWNED_HOOKS, discover};
pub use group::GroupHints;
pub use plugin::declared as plugin_hooks;
pub use provision::provision;
