//! Which rows `u` updates, and which rows are known to have an update.

use super::InstalledTab;
use skillz_harness::model::Installed;
use skillz_harness::update::can_update;

impl InstalledTab {
    pub(super) fn is_outdated(&self, id: &str) -> bool {
        self.outdated.contains(id)
    }

    /// The installed, updatable items of the highlighted row: itself, or everything under it.
    pub(super) fn updatable_ids(&self) -> Vec<String> {
        self.acted_on()
            .iter()
            .filter_map(|id| self.installed.iter().find(|item| &item.id == id))
            .flat_map(Installed::parts)
            .filter(|item| can_update(item))
            .map(|item| item.id.clone())
            .collect()
    }
}
