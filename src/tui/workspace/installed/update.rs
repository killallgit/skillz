//! Which rows `u` updates, and which rows are known to have an update.

use super::InstalledTab;
use super::rows::{Row, State};
use crate::update::can_update;

impl InstalledTab {
    pub(super) fn is_outdated(&self, state: &State) -> bool {
        matches!(state, State::Installed(id) if self.outdated.contains(id))
    }

    /// The installed, updatable items of the highlighted row: itself, or everything under it.
    pub(super) fn updatable_ids(&self) -> Vec<String> {
        let Some(current) = self.outline.current() else {
            return vec![];
        };
        let visible = self.outline.visible();
        let depth = self.outline.depth(current);
        let under = (current + 1..self.rows.len())
            .take_while(|&at| self.outline.depth(at) > depth)
            .filter(|at| visible.contains(at));
        std::iter::once(current)
            .chain(under)
            .filter_map(|at| match &self.rows[at] {
                Row::Item(item) => installed_id(&item.state),
                _ => None,
            })
            .filter(|id| {
                self.installed
                    .iter()
                    .any(|item| &item.id == id && can_update(item))
            })
            .collect()
    }
}

fn installed_id(state: &State) -> Option<String> {
    match state {
        State::Installed(id) => Some(id.clone()),
        _ => None,
    }
}
