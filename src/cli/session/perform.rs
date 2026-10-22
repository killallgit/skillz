//! Doing what the workspace asked for.

use super::Session;
use super::change::{Change, change};
use super::notice::Notice;
use super::selection::Selection;
use crate::install::{InstallRequest, SkillSelection};
use crate::model::Scope;
use crate::tui::workspace::Act;

impl Session<'_, '_> {
    /// What the toast should say, if anything.
    pub(in crate::cli) fn perform(&self, act: Act) -> anyhow::Result<Option<Notice>> {
        let what = match &act {
            Act::Quit => return Ok(None),
            Act::Remove { scope, ids } => {
                let picked = Selection {
                    install: &[],
                    remove: ids,
                };
                return change(self, *scope, &Change::Selection(&picked));
            }
            Act::Update { scope, ids } => return change(self, *scope, &Change::Update(ids)),
            Act::Apply { profile } => Change::Apply(profile),
            Act::InstallSource { scope, source } => {
                return self.install_source(&InstallRequest {
                    source: source.clone(),
                    cwd: self.deps.cwd.clone(),
                    agents: self.deps.adapters.iter().map(|a| a.agent()).collect(),
                    scope: *scope,
                    selection: SkillSelection::Interactive,
                });
            }
            Act::LibraryAdd { spec } => return self.library_add(spec),
            Act::LibraryImport => return self.library_import(),
            Act::LibraryRemove(entry) => return self.library_remove(entry),
            Act::ApplyExact { profile } => Change::ApplyExact(profile),
            Act::NewProfile { name } => return self.create_profile(name).map(Some),
            Act::SaveCurrent { name } => return self.save_current(name).map(Some),
            Act::SetEntries { profile, entries } => {
                return self.set_entries(profile, entries).map(Some);
            }
            Act::DeleteProfile { name } => return self.delete_profile(name).map(Some),
        };
        change(self, Scope::Project, &what)
    }
}
