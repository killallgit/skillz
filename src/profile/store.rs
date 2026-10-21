use std::fs;
use std::path::PathBuf;

use anyhow::Context;

use super::identity::overlay;
use super::{Entry, Profile, check_name};
use crate::atomic::write_atomic;
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

/// A directory of `<name>.toml` profiles.
pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(dir: PathBuf) -> Self {
        Store { dir }
    }

    /// `<skillz dir>/profiles`.
    pub fn in_env(env: &Env) -> Self {
        Store::new(env.skillz_dir().join("profiles"))
    }

    fn path(&self, name: &str) -> anyhow::Result<PathBuf> {
        check_name(name)?;
        Ok(self.dir.join(format!("{name}.toml")))
    }

    /// Profile names, sorted. A missing directory has none.
    pub fn names(&self) -> anyhow::Result<Vec<String>> {
        let read = match fs::read_dir(&self.dir) {
            Ok(read) => read,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(e).with_context(|| format!("reading {}", self.dir.display())),
        };
        let mut names = Vec::new();
        for entry in read {
            let path = entry
                .with_context(|| format!("reading {}", self.dir.display()))?
                .path();
            names.extend(profile_name(&path).filter(|n| check_name(n).is_ok()));
        }
        names.sort();
        Ok(names)
    }

    pub fn load(&self, name: &str) -> anyhow::Result<Profile> {
        let path = self.path(name)?;
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        Profile::parse(name, &text).with_context(|| path.display().to_string())
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

    pub fn save(&self, profile: &Profile) -> anyhow::Result<()> {
        profile.validate()?;
        let path = self.path(&profile.name)?;
        write_atomic(&path, &profile.to_text()?)
    }

    /// The profiles that list `name` in `extends`. Unreadable ones are skipped.
    fn extended_by(&self, name: &str) -> anyhow::Result<Vec<String>> {
        Ok(self
            .names()?
            .into_iter()
            .filter(|other| other != name)
            .filter(|other| {
                self.load(other)
                    .is_ok_and(|p| p.extends.iter().any(|base| base == name))
            })
            .collect())
    }

    /// Missing is fine. A profile another one extends is refused with `StillExtended`.
    pub fn delete(&self, name: &str) -> anyhow::Result<()> {
        let by = self.extended_by(name)?;
        if !by.is_empty() {
            return Err(StillExtended {
                name: name.into(),
                by,
            }
            .into());
        }
        match fs::remove_file(self.path(name)?) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        }
    }
}

/// The profile a `<name>.toml` path holds; `None` for any other file.
fn profile_name(path: &std::path::Path) -> Option<String> {
    if path.extension()? != "toml" {
        return None;
    }
    path.file_stem()?.to_str().map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().join("profiles"));
        (dir, store)
    }

    #[test]
    fn saved_profiles_are_listed_sorted_and_load_back() {
        let (dir, store) = store();
        let mut web = Profile::new("web");
        web.description = Some("frontend".into());

        store.save(&web).unwrap();
        store.save(&Profile::new("a")).unwrap();
        fs::write(dir.path().join("profiles/notes.txt"), "x").unwrap();

        assert_eq!(store.names().unwrap(), ["a", "web"]);
        assert_eq!(store.load("web").unwrap(), web);
    }

    #[test]
    fn an_unreadable_profile_directory_is_an_error_not_an_empty_list() {
        let (dir, store) = store();
        fs::write(dir.path().join("profiles"), "not a directory").unwrap();

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
    fn a_missing_directory_has_no_profiles() {
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
