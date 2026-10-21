use std::fs;
use std::path::PathBuf;

use anyhow::Context;

use super::{Profile, check_name};
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
