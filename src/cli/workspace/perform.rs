//! Doing what the workspace asked for.

use super::Session;
use super::change::{Change, change};
use super::notice::Notice;
use crate::tui::workspace::Act;

impl Session<'_, '_> {
    /// What the toast should say, if anything.
    pub(super) fn perform(&self, act: Act) -> anyhow::Result<Option<Notice>> {
        let what = match &act {
            Act::Quit => return Ok(None),
            Act::Install(entry) => Change::Install(entry),
            Act::Remove { id } => Change::Remove(id),
            Act::Apply { profile } => Change::Apply(profile),
            Act::LibraryAdd { spec } => return self.library_add(spec),
            Act::LibraryImport => return self.library_import(),
            Act::LibraryRemove(entry) => return self.library_remove(entry),
        };
        change(self.deps, &self.project, &what)
    }
}
