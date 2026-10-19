//! Minimal terminal widgets: a multi-select list and a collapsible tree, both with `/` filter.
//! Each widget is a pure state machine (`State::handle(key)`) plus a thin render/event loop.

pub mod select;
pub mod tree_view;
