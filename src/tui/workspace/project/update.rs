//! Which rows `u` updates, and which rows are known to have an update.

use super::ProjectTab;
use super::rows::{Row, State};
use crate::update::can_update;

impl ProjectTab {
    pub(super) fn is_outdated(&self, state: &State) -> bool {
        matches!(state, State::Installed(id) if self.outdated.contains(id))
    }

    /// The installed, updatable items of the highlighted row: itself, or its section's.
    pub(super) fn updatable_ids(&self) -> Vec<String> {
        let Some(current) = self.outline.current() else {
            return vec![];
        };
        let visible = self.outline.visible();
        let covered: Vec<&Row> = match &self.rows[current] {
            Row::Item(_) => vec![&self.rows[current]],
            Row::Section { .. } => (current + 1..self.rows.len())
                .take_while(|&at| matches!(self.rows[at], Row::Item(_)))
                .filter(|at| visible.contains(at))
                .map(|at| &self.rows[at])
                .collect(),
        };
        covered
            .into_iter()
            .filter_map(|row| match row {
                Row::Item(item) => match &item.state {
                    State::Installed(id) => Some(id.clone()),
                    State::Available(_) => None,
                },
                Row::Section { .. } => None,
            })
            .filter(|id| {
                self.installed
                    .iter()
                    .any(|item| &item.id == id && can_update(item))
            })
            .collect()
    }
}
