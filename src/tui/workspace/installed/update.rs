//! Which rows `u` updates, and which rows are known to have an update.

use super::InstalledTab;
use super::rows::State;
use crate::model::Augmentation;
use crate::update::can_update;

impl InstalledTab {
    pub(super) fn is_outdated(&self, state: &State) -> bool {
        matches!(state, State::Installed(id) if self.outdated.contains(id))
    }

    /// The installed, updatable items of the highlighted row: itself, or everything under it.
    pub(super) fn updatable_ids(&self) -> Vec<String> {
        self.acted_on()
            .iter()
            .filter_map(|state| match state {
                State::Installed(id) => self.installed.iter().find(|item| &item.id == id),
                State::Available(_) => None,
            })
            .flat_map(Augmentation::parts)
            .filter(|item| can_update(item))
            .map(|item| item.id.clone())
            .collect()
    }
}
