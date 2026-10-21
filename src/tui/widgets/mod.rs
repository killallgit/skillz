//! Reusable widgets. Each is a pure state type plus a render fn that takes a `Theme`.

pub mod confirm;
pub mod empty;
pub mod input;
pub mod keybar;
pub mod list;
pub mod modal;
pub mod panel;
pub mod table;
pub mod tabs;
#[cfg(test)]
pub mod testing;
pub mod toast;

/// What a navigation widget did with a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Ignored,
    Changed,
}
