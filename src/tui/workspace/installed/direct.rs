//! Removing the highlighted row at once.

use super::InstalledTab;
use crate::tui::workspace::act::Act;

impl InstalledTab {
    /// Remove the highlighted item, or everything its package provides.
    pub(super) fn remove_current(&mut self) -> Option<Act> {
        let ids = self.acted_on();
        let scope = self.scope;
        (!ids.is_empty()).then_some(Act::Remove { scope, ids })
    }
}
