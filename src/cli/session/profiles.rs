//! Changes to the stored profiles: creating, building and deleting.

use super::Session;
use super::notice::Notice;
use crate::app::profile::Profile;
use skillz_harness::Spec;
use skillz_harness::model::Scope;

/// What the applied profile of the user level is remembered under.
const USER_LEVEL: &str = "user-level";

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

    /// The profile last applied at `scope`: to this project, or at user level. Deleting a
    /// profile forgets it. Only tests read the record back: nothing shows it since the profile
    /// header went with the old tabs.
    #[cfg(test)]
    pub(in crate::cli) fn applied(&self, scope: Scope) -> Option<String> {
        self.store
            .read()
            .ok()?
            .applied
            .remove(&self.applied_key(scope))
    }

    pub(in crate::cli) fn remember_applied(&self, scope: Scope, name: &str) -> anyhow::Result<()> {
        let key = self.applied_key(scope);
        self.store.update(|document| {
            document.applied.insert(key, name.into());
            Ok(())
        })
    }

    /// A project is remembered by its path; the user level by a word no path can be.
    fn applied_key(&self, scope: Scope) -> String {
        match scope {
            Scope::Project => self.project.display().to_string(),
            Scope::Global => USER_LEVEL.into(),
        }
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
