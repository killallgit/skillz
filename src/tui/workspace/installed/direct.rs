//! Removing the highlighted row at once.

use super::InstalledTab;
use crate::workspace::Act;

impl InstalledTab {
    /// Remove the highlighted item, or everything its package provides; on a section, mark every
    /// item in it for removal instead.
    pub(super) fn remove_current(&mut self) -> Option<Act> {
        if let Some(ids) = self.section_ids() {
            self.pending.toggle_all(&ids);
            return None;
        }
        let ids = self.acted_on();
        match ids.is_empty() {
            true => None,
            false => Some(Act::Remove {
                scope: self.scope,
                ids,
            }),
        }
    }
}
