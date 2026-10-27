//! Doing what the workspace asked for.

use super::Session;
use super::change::{Change, change};
use super::notice::Notice;
use super::selection::Selection;
use crate::app::install::{InstallRequest, SkillSelection};
use crate::app::workspace::Act;
use skillz_harness::Spec;
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
            Act::Apply { scope, profile } => self.apply(*scope, profile, &Change::Apply(profile)),
            Act::ApplyExact { scope, profile } => {
                self.apply(*scope, profile, &Change::ApplyExact(profile))
            }
            Act::InstallSource { scope, source } => self.install_source(&InstallRequest {
                source: source.clone(),
                cwd: self.deps.cwd.clone(),
                agents: self.deps.agents_or_all(&[]),
                scope: *scope,
                selection: SkillSelection::Interactive,
            }),
            Act::LibraryAdd { spec } => self.library_add(spec),
            Act::LibraryInstall { scope, entry } => self.library_install(*scope, entry),
            Act::LibraryRemove(entry) => self.library_remove(entry),
            Act::CreateProfile { name, entries } => self.create_profile(name, entries).map(Some),
            Act::SaveCurrent { name } => {
                let saved = self.save_current(name)?;
                self.remember_applied(Scope::Project, name)?;
                Ok(Some(saved))
            }
            Act::SetEntries { profile, entries } => self.set_entries(profile, entries).map(Some),
            Act::DeleteProfile { name } => self.delete_profile(name).map(Some),
        }
    }

    /// Install a library entry. A skill that names its skills is installed from its source like
    /// any other source, so a plugin the source offers is used; every other entry is a plain
    /// change.
    fn library_install(&self, scope: Scope, entry: &Spec) -> anyhow::Result<Option<Notice>> {
        if let Spec::Skill {
            source,
            names,
            agents,
        } = entry
            && !names.is_empty()
        {
            return self.install_source(&InstallRequest {
                source: source.clone(),
                cwd: self.deps.cwd.clone(),
                agents: self.deps.agents_or_all(agents),
                scope,
                selection: SkillSelection::Patterns(names.clone()),
            });
        }
        let picked = Selection {
            install: std::slice::from_ref(entry),
            remove: &[],
            only: None,
        };
        change(self, scope, &Change::Selection(&picked))
    }

    /// Apply `profile` at `scope`, and remember it as that level's once the change went through.
    pub(in crate::cli) fn apply(
        &self,
        scope: Scope,
        profile: &str,
        what: &Change,
    ) -> anyhow::Result<Option<Notice>> {
        let notice = change(self, scope, what)?;
        if matches!(notice, Some(Notice::Applied(_) | Notice::UpToDate)) {
            self.remember_applied(scope, profile)?;
        }
        Ok(notice)
    }
}
