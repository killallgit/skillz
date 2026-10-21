//! Reusable widgets. Each is a pure state type plus a render fn that takes a `Theme`.

pub(crate) mod confirm;
pub(crate) mod empty;
pub(crate) mod input;
pub(crate) mod keybar;
pub(crate) mod list;
pub(crate) mod modal;
pub(crate) mod panel;
#[allow(dead_code, reason = "wired back in by the next TUI bead")]
pub(crate) mod path_picker;
pub(crate) mod scroll;
pub(crate) mod table;
pub(crate) mod tabs;
#[cfg(test)]
pub(crate) mod testing;
pub(crate) mod toast;

/// What a navigation widget did with a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Ignored,
    Changed,
}
