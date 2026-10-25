//! Doing what the workspace asked for.

use super::Session;
use super::change::{Change, change};
use super::notice::Notice;
use super::selection::Selection;
use crate::install::{InstallRequest, SkillSelection};
use crate::workspace::Act;
use skillz_harness::model::Scope;

impl Session<'_, '_> {
    /// What the toast should say, if anything.
    pub(in crate::cli) fn perform(&self, act: Act) -> anyhow::Result<Option<Notice>> {
        match &act {
            Act::Quit => Ok(None),
            Act::Remove { scope, ids } => {
                let picked = Selection {
                    install: &[],
                    remove: ids,
                    only: None,
                };
                change(self, *scope, &Change::Selection(&picked))
            }
            Act::Subset { install, remove } => {
                let picked = Selection {
                    install,
                    remove,
                    only: None,
                };
                change(self, Scope::Project, &Change::Selection(&picked))
            }
            Act::Update { scope, ids } => change(self, *scope, &Change::Update(ids)),
            Act::Apply { profile } => self.apply(profile, &Change::Apply(profile)),
            Act::ApplyExact { profile } => self.apply(profile, &Change::ApplyExact(profile)),
            Act::InstallSource { scope, source } => self.install_source(&InstallRequest {
                source: source.clone(),
                cwd: self.deps.cwd.clone(),
                agents: self.deps.adapters.iter().map(|a| a.agent()).collect(),
                scope: *scope,
                selection: SkillSelection::Interactive,
            }),
            Act::LibraryAdd { spec } => self.library_add(spec),
            Act::LibraryImport => self.library_import(),
            Act::LibraryRemove(entry) => self.library_remove(entry),
            Act::CreateProfile { name, entries } => self.create_profile(name, entries).map(Some),
            Act::SaveCurrent { name } => {
                let saved = self.save_current(name)?;
                self.remember_applied(name)?;
                Ok(Some(saved))
            }
            Act::SetEntries { profile, entries } => self.set_entries(profile, entries).map(Some),
            Act::DeleteProfile { name } => self.delete_profile(name).map(Some),
        }
    }

    /// Apply `profile`, and remember it as the project's once the change went through.
    pub(in crate::cli) fn apply(
        &self,
        profile: &str,
        what: &Change,
    ) -> anyhow::Result<Option<Notice>> {
        let notice = change(self, Scope::Project, what)?;
        if matches!(notice, Some(Notice::Applied(_) | Notice::UpToDate)) {
            self.remember_applied(profile)?;
        }
        Ok(notice)
    }
}
