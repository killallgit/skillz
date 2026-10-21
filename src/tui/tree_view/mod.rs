//! The terminal list: one collapsible, markable tree with `/` filter, used for browsing and
//! for every picker. `model` holds the data, `input` the keys, `draw` the screen.

mod draw;
mod input;
mod marks;
mod menu;
mod model;
#[cfg(test)]
mod tests;

pub use input::Outcome;
pub use model::TreeState;

use super::terminal::event_loop;
use super::theme::Theme;
use crate::prompt::{Choice, Picked};
use crate::tree::Node;

/// Browse `nodes`, mark groups or items, and pick an action; `None` when the user quits.
pub fn browse(
    title: &str,
    nodes: &[Node],
    theme: &Theme,
) -> anyhow::Result<Option<(Choice, Picked)>> {
    event_loop(
        &mut TreeState::new(nodes),
        |s, f| s.draw(title, f, theme),
        |s, k| match s.handle(k) {
            Outcome::Continue => None,
            Outcome::Quit => Some(None),
            Outcome::Accept(choice, picked) => Some(Some((choice, picked))),
        },
    )
}

/// Pick items from `nodes` (marking groups or items); the chosen `Node::item` indices,
/// ascending. Empty when the user quits.
pub fn pick(title: &str, nodes: &[Node], theme: &Theme) -> anyhow::Result<Vec<usize>> {
    event_loop(
        &mut TreeState::picker(nodes),
        |s, f| s.draw(title, f, theme),
        |s, k| match s.handle(k) {
            Outcome::Continue => None,
            Outcome::Quit => Some(vec![]),
            Outcome::Accept(_, picked) => Some(picked.item_indices(nodes)),
        },
    )
}
