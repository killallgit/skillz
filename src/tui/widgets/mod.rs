//! Reusable widgets. Each is a pure state type plus a render fn that takes a `Theme`.

pub(crate) mod confirm;
pub(crate) mod empty;
pub(crate) mod input;
pub(crate) mod keybar;
pub(crate) mod list;
pub(crate) mod modal;
pub(crate) mod outline;
pub(crate) mod panel;
pub(crate) mod tabs;
#[cfg(test)]
pub(crate) mod testing;
pub(crate) mod text_modal;
pub(crate) mod toast;

/// What a navigation widget did with a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Ignored,
    Changed,
}
