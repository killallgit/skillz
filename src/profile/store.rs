use std::fs;
use std::path::PathBuf;

use anyhow::Context;

use super::{Entry, Profile, check_name};
use crate::env::Env;

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
        let Ok(read) = fs::read_dir(&self.dir) else {
            return Ok(vec![]);
        };
        let mut names: Vec<String> = read
            .flatten()
            .filter_map(|e| profile_name(&e.path()))
            .filter(|n| check_name(n).is_ok())
            .collect();
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
    /// with its own profile's agents filled in. A base that is missing, or that leads back to
    /// itself, is an error.
    pub fn load_flat(&self, name: &str) -> anyhow::Result<Profile> {
        let mut profile = self.load(name)?;
        let mut entries = self.base_entries(&profile, &mut vec![name.to_string()])?;
        entries.extend(profile.resolved());
        profile.entries = entries;
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
            entries.extend(self.base_entries(&loaded, chain)?);
            chain.pop();
            entries.extend(loaded.resolved());
        }
        Ok(entries)
    }

    pub fn save(&self, profile: &Profile) -> anyhow::Result<()> {
        profile.validate()?;
        let path = self.path(&profile.name)?;
        fs::create_dir_all(&self.dir)?;
        fs::write(&path, profile.to_text()?).with_context(|| format!("writing {}", path.display()))
    }

    /// Missing is fine.
    pub fn delete(&self, name: &str) -> anyhow::Result<()> {
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
    fn deleting_twice_is_fine_and_the_profile_is_gone() {
        let (_dir, store) = store();
        store.save(&Profile::new("web")).unwrap();

        store.delete("web").unwrap();
        store.delete("web").unwrap();

        assert!(store.names().unwrap().is_empty());
        assert!(store.load("web").is_err());
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
