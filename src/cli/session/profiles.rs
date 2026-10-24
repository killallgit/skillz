//! Changes to the stored profiles: creating, building, saving from the project and deleting.

use super::Session;
use super::notice::Notice;
use crate::cli::profile::save_profile;
use crate::cli::scope::{complete_installed, project_env};
use crate::model::Scope;
use crate::profile::{Profile, Spec};

impl Session<'_, '_> {
    pub(in crate::cli) fn create_profile(
        &self,
        name: &str,
        entries: &[Spec],
    ) -> anyhow::Result<Notice> {
        let exists = self.profiles.names()?.iter().any(|n| n == name);
        anyhow::ensure!(!exists, "profile {name} exists");
        let mut profile = Profile::new(name);
        profile.entries = entries.to_vec();
        self.profiles.save(&profile)?;
        Ok(Notice::Created(name.into()))
    }

    /// A profile of what the project has installed; what cannot be reproduced is reported.
    pub(in crate::cli) fn save_current(&self, name: &str) -> anyhow::Result<Notice> {
        let env = project_env(self.deps, &self.project);
        let items = complete_installed(self.deps, &env, Scope::Project)?;
        let saved = save_profile(&self.profiles, &items, name, false)?;
        Ok(Notice::Saved {
            name: name.into(),
            skipped: saved.skipped.into_iter().map(|s| s.label).collect(),
        })
    }

    pub(in crate::cli) fn set_entries(
        &self,
        name: &str,
        entries: &[Spec],
    ) -> anyhow::Result<Notice> {
        let mut profile = self.profiles.load(name)?;
        profile.entries = entries.to_vec();
        self.profiles.save(&profile)?;
        Ok(Notice::Updated(name.into()))
    }

    /// The profile last applied to this project; deleting a profile forgets it here.
    pub(in crate::cli) fn applied(&self) -> Option<String> {
        self.store.read().ok()?.applied.remove(&self.project_key())
    }

    pub(in crate::cli) fn remember_applied(&self, name: &str) -> anyhow::Result<()> {
        let key = self.project_key();
        self.store.update(|document| {
            document.applied.insert(key, name.into());
            Ok(())
        })
    }

    fn project_key(&self) -> String {
        self.project.display().to_string()
    }

    pub(in crate::cli) fn delete_profile(&self, name: &str) -> anyhow::Result<Notice> {
        self.profiles.delete(name)?;
        self.store.update(|document| {
            document.applied.retain(|_, applied| applied != name);
            Ok(())
        })?;
        Ok(Notice::Deleted(name.into()))
    }
}

#[cfg(test)]
mod tests;
