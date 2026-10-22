//! Changes to the stored profiles: creating, building, saving from the project and deleting.

use super::Session;
use super::notice::Notice;
use crate::cli::profile::save_profile;
use crate::cli::scope::{complete_installed, project_env};
use crate::model::Scope;
use crate::profile::{Entry, Profile};

impl Session<'_, '_> {
    pub(in crate::cli) fn create_profile(&self, name: &str) -> anyhow::Result<Notice> {
        let exists = self.profiles.names()?.iter().any(|n| n == name);
        anyhow::ensure!(!exists, "profile {name} exists");
        self.profiles.save(&Profile::new(name))?;
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
        entries: &[Entry],
    ) -> anyhow::Result<Notice> {
        let mut profile = self.profiles.load(name)?;
        profile.entries = entries.to_vec();
        self.profiles.save(&profile)?;
        Ok(Notice::Updated(name.into()))
    }

    pub(in crate::cli) fn delete_profile(&self, name: &str) -> anyhow::Result<Notice> {
        self.profiles.delete(name)?;
        Ok(Notice::Deleted(name.into()))
    }
}

#[cfg(test)]
mod tests;
