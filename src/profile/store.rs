use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Context;

use super::identity::overlay;
use super::{Profile, check_name};
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
        Store::new(crate::app_paths::store_path(env))
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
        let profile = self
            .all()?
            .into_iter()
            .find(|profile| profile.name == name)
            .with_context(|| format!("no profile {name}"))?;
        profile.validate()?;
        Ok(profile)
    }

    /// Every profile that loads, flattened as by `load_flat`; the unreadable ones are left out.
    pub fn readable(&self) -> anyhow::Result<Vec<Profile>> {
        let profiles = self.all()?;
        let mut names: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();
        names.sort();
        let mut resolver = Resolver::new(&profiles);
        Ok(names
            .into_iter()
            .filter_map(|name| resolver.resolve(name, &mut vec![]).ok())
            .collect())
    }

    /// Resolve inheritance from one store snapshot. Later bases and the profile's own entries win.
    pub fn load_flat(&self, name: &str) -> anyhow::Result<Profile> {
        check_name(name)?;
        Resolver::new(&self.all()?).resolve(name, &mut vec![])
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

struct Resolver<'a> {
    profiles: &'a [Profile],
    resolved: HashMap<String, Profile>,
}

impl<'a> Resolver<'a> {
    fn new(profiles: &'a [Profile]) -> Self {
        Self {
            profiles,
            resolved: HashMap::new(),
        }
    }

    fn resolve(&mut self, name: &str, chain: &mut Vec<String>) -> anyhow::Result<Profile> {
        if let Some(profile) = self.resolved.get(name) {
            return Ok(profile.clone());
        }
        anyhow::ensure!(
            !chain.iter().any(|base| base == name),
            "profile {name} extends itself"
        );
        let mut profile = self
            .profiles
            .iter()
            .find(|p| p.name == name)
            .cloned()
            .with_context(|| format!("no profile {name}"))?;
        profile.validate()?;
        chain.push(name.to_string());
        let inherited = (|| {
            let mut entries = vec![];
            for base in &profile.extends {
                entries = overlay(entries, &self.resolve(base, chain)?.entries);
            }
            Ok::<_, anyhow::Error>(entries)
        })();
        chain.pop();
        profile.entries = overlay(inherited?, &profile.resolved());
        profile.extends.clear();
        self.resolved.insert(name.into(), profile.clone());
        Ok(profile)
    }
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
    use crate::harness::Spec;

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

    fn skill(name: &str) -> Spec {
        Spec::Skill {
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
                Spec::Skill { names, .. } => names.clone(),
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
