use std::path::PathBuf;

use anyhow::Context;

use super::identity::overlay;
use super::{Entry, Profile, check_name};
use crate::env::Env;

/// A profile cannot be removed while others start from it.
#[derive(Debug, PartialEq, Eq)]
pub struct StillExtended {
    pub name: String,
    pub by: Vec<String>,
}

impl std::fmt::Display for StillExtended {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "profile {} is extended by {}",
            self.name,
            self.by.join(", ")
        )
    }
}

impl std::error::Error for StillExtended {}

/// The profiles section of the global store.
pub struct Store {
    store: crate::store::Store,
}

impl Store {
    pub fn new(path: PathBuf) -> Self {
        Store {
            store: crate::store::Store::new(path),
        }
    }

    pub fn in_env(env: &Env) -> Self {
        Store::new(env.store_path())
    }

    fn all(&self) -> anyhow::Result<Vec<Profile>> {
        Ok(self.store.read()?.profiles)
    }

    /// Profile names, sorted. A missing store has none.
    pub fn names(&self) -> anyhow::Result<Vec<String>> {
        let mut names: Vec<String> = self.all()?.into_iter().map(|p| p.name).collect();
        names.sort();
        Ok(names)
    }

    pub fn load(&self, name: &str) -> anyhow::Result<Profile> {
        check_name(name)?;
        let profile = find(self.all()?, name).with_context(|| format!("no profile {name}"))?;
        profile.validate()?;
        Ok(profile)
    }

    /// Every profile that loads, flattened as by `load_flat`; the unreadable ones are left out.
    pub fn readable(&self) -> anyhow::Result<Vec<Profile>> {
        let names = self.names()?;
        Ok(names
            .iter()
            .filter_map(|name| self.load_flat(name).ok())
            .collect())
    }

    /// The profile with every profile it extends folded in: their entries first, in order, each
    /// with its own profile's agents filled in. An entry replaces any inherited one with the same
    /// identity (see `identity`). A base that is missing, or that leads back to itself, is an
    /// error.
    pub fn load_flat(&self, name: &str) -> anyhow::Result<Profile> {
        let mut profile = self.load(name)?;
        let inherited = self.base_entries(&profile, &mut vec![name.to_string()])?;
        profile.entries = overlay(inherited, &profile.resolved());
        profile.extends.clear();
        Ok(profile)
    }

    /// The resolved entries of everything `profile` extends. `chain` is the profiles being
    /// expanded, to catch cycles.
    fn base_entries(
        &self,
        profile: &Profile,
        chain: &mut Vec<String>,
    ) -> anyhow::Result<Vec<Entry>> {
        let mut entries = vec![];
        for base in &profile.extends {
            anyhow::ensure!(!chain.contains(base), "profile {base} extends itself");
            let loaded = self.load(base)?;
            chain.push(base.clone());
            let inherited = self.base_entries(&loaded, chain)?;
            chain.pop();
            entries = overlay(entries, &overlay(inherited, &loaded.resolved()));
        }
        Ok(entries)
    }

    /// Add the profile, or replace the one of the same name.
    pub fn save(&self, profile: &Profile) -> anyhow::Result<()> {
        profile.validate()?;
        self.store.update(|document| {
            document.profiles.retain(|p| p.name != profile.name);
            document.profiles.push(profile.clone());
            Ok(())
        })
    }

    /// Missing is fine. A profile another one extends is refused with `StillExtended`.
    pub fn delete(&self, name: &str) -> anyhow::Result<()> {
        check_name(name)?;
        self.store.update(|document| {
            let by = extended_by(&document.profiles, name);
            if !by.is_empty() {
                return Err(StillExtended {
                    name: name.into(),
                    by,
                }
                .into());
            }
            document.profiles.retain(|p| p.name != name);
            Ok(())
        })
    }
}

fn find(profiles: Vec<Profile>, name: &str) -> Option<Profile> {
    profiles.into_iter().find(|p| p.name == name)
}

/// The profiles that list `name` in `extends`.
fn extended_by(profiles: &[Profile], name: &str) -> Vec<String> {
    let mut by: Vec<String> = profiles
        .iter()
        .filter(|p| p.name != name && p.extends.iter().any(|base| base == name))
        .map(|p| p.name.clone())
        .collect();
    by.sort();
    by
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().join("store.json"));
        (dir, store)
    }

    #[test]
    fn saved_profiles_are_listed_sorted_and_load_back() {
        let (_dir, store) = store();
        let mut web = Profile::new("web");
        web.description = Some("frontend".into());

        store.save(&web).unwrap();
        store.save(&Profile::new("a")).unwrap();

        assert_eq!(store.names().unwrap(), ["a", "web"]);
        assert_eq!(store.load("web").unwrap(), web);
    }

    #[test]
    fn a_corrupt_store_is_an_error_not_an_empty_list() {
        let (dir, store) = store();
        std::fs::write(dir.path().join("store.json"), "not json").unwrap();

        assert!(store.names().is_err());
    }

    #[test]
    fn deleting_twice_is_fine_and_the_profile_is_gone() {
        let (_dir, store) = store();
        store.save(&Profile::new("web")).unwrap();

        store.delete("web").unwrap();
        store.delete("web").unwrap();

        assert!(store.names().unwrap().is_empty());
        assert!(store.load("web").is_err());
    }

    #[test]
    fn a_profile_another_extends_cannot_be_removed_until_that_one_goes() {
        let (_dir, store) = store();
        store.save(&Profile::new("base")).unwrap();
        let mut web = Profile::new("web");
        web.extends = vec!["base".into()];
        store.save(&web).unwrap();

        let refused = store.delete("base").unwrap_err();
        store.delete("web").unwrap();
        store.delete("base").unwrap();

        // The refusal is the typed error naming the dependents, and the store ends empty.
        let typed = refused.downcast_ref::<StillExtended>().unwrap();
        assert_eq!(typed.by, ["web"]);
        assert!(store.names().unwrap().is_empty());
    }

    fn skill(name: &str) -> Entry {
        Entry::Skill {
            source: "o/r".into(),
            names: vec![name.into()],
            agents: vec![],
        }
    }

    fn saved(store: &Store, name: &str, extends: &[&str], skills: &[&str]) {
        let mut p = Profile::new(name);
        p.extends = extends.iter().map(|b| b.to_string()).collect();
        p.entries = skills.iter().map(|s| skill(s)).collect();
        store.save(&p).unwrap();
    }

    fn skill_names(profile: &Profile) -> Vec<String> {
        profile
            .entries
            .iter()
            .flat_map(|e| match e {
                Entry::Skill { names, .. } => names.clone(),
                _ => vec![],
            })
            .collect()
    }

    #[test]
    fn a_profile_starts_from_the_entries_of_the_ones_it_extends() {
        let (_dir, store) = store();
        saved(&store, "base", &[], &["tdd"]);
        saved(&store, "mid", &["base"], &["grill"]);
        saved(&store, "web", &["mid"], &["react"]);

        let flat = store.load_flat("web").unwrap();

        assert_eq!(skill_names(&flat), ["tdd", "grill", "react"]);
        assert!(flat.extends.is_empty());
    }

    #[test]
    fn a_child_entry_replaces_the_inherited_one_with_the_same_identity() {
        let (_dir, store) = store();
        saved(&store, "base", &[], &["tdd", "grill"]);
        saved(&store, "web", &["base"], &["tdd"]);

        let flat = store.load_flat("web").unwrap();

        // Assert tdd appears once, after the surviving inherited skill.
        assert_eq!(skill_names(&flat), ["grill", "tdd"]);
    }

    #[test]
    fn of_two_bases_the_later_one_wins() {
        let (_dir, store) = store();
        let mut one = Profile::new("one");
        one.agents = vec![crate::model::Agent::Claude];
        one.entries = vec![skill("tdd")];
        store.save(&one).unwrap();
        let mut two = Profile::new("two");
        two.agents = vec![crate::model::Agent::Codex];
        two.entries = vec![skill("tdd")];
        store.save(&two).unwrap();
        saved(&store, "web", &["one", "two"], &[]);

        let flat = store.load_flat("web").unwrap();

        // Assert one entry is left, with the later base's agents.
        assert_eq!(flat.entries.len(), 1);
        assert_eq!(flat.entries[0].agents(), [crate::model::Agent::Codex]);
    }

    #[test]
    fn a_base_keeps_the_agents_of_its_own_profile() {
        let (_dir, store) = store();
        let mut base = Profile::new("base");
        base.agents = vec![crate::model::Agent::Codex];
        base.entries = vec![skill("tdd")];
        store.save(&base).unwrap();
        saved(&store, "web", &["base"], &[]);

        let flat = store.load_flat("web").unwrap();

        assert_eq!(flat.entries[0].agents(), [crate::model::Agent::Codex]);
    }

    #[test]
    fn a_missing_base_or_a_cycle_is_an_error() {
        let (_dir, store) = store();
        saved(&store, "web", &["nope"], &[]);
        saved(&store, "a", &["b"], &[]);
        saved(&store, "b", &["a"], &[]);

        assert!(store.load_flat("web").is_err());
        assert!(store.load_flat("a").is_err());
    }

    #[test]
    fn a_profile_cannot_extend_something_that_is_not_a_name() {
        let (_dir, store) = store();
        let mut p = Profile::new("web");
        p.extends = vec!["../x".into()];

        assert!(store.save(&p).is_err());
    }

    #[test]
    fn a_missing_store_has_no_profiles() {
        let (_dir, store) = store();

        assert!(store.names().unwrap().is_empty());
    }

    #[test]
    fn saving_an_invalid_profile_writes_nothing() {
        let (_dir, store) = store();
        let mut p = Profile::new("web");
        p.agents.clear();

        assert!(store.save(&p).is_err());
        assert!(store.names().unwrap().is_empty());
    }
}
