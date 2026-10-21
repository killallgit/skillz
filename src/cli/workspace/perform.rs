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
            Act::ApplyExact { profile } => Change::ApplyExact(profile),
            Act::NewProfile { name } => return self.create_profile(name).map(Some),
            Act::SaveCurrent { name } => return self.save_current(name).map(Some),
            Act::SetEntries { profile, entries } => {
                return self.set_entries(profile, entries).map(Some);
            }
            Act::DeleteProfile { name } => return self.delete_profile(name).map(Some),
        };
        change(self.deps, &self.project, &what)
    }
}
