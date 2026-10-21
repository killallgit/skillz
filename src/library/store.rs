use std::path::PathBuf;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use super::catalog::Catalog;
use super::item::LibraryItem;
use crate::atomic::write_atomic;
use crate::env::Env;
use crate::read_optional::read_optional;

/// The library file exists but is not a valid library; it is left untouched.
#[derive(Debug, PartialEq, Eq)]
pub struct Corrupt {
    pub path: PathBuf,
}

impl std::fmt::Display for Corrupt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} is not a valid library", self.path.display())
    }
}

impl std::error::Error for Corrupt {}

#[derive(Serialize, Deserialize)]
struct File {
    #[serde(default, rename = "item")]
    items: Vec<LibraryItem>,
}

/// A library kept as one toml file at an injected path.
pub struct Store {
    path: PathBuf,
}

impl Store {
    pub fn new(path: PathBuf) -> Self {
        Store { path }
    }

    /// `<skillz dir>/library.toml`.
    pub fn in_env(env: &Env) -> Self {
        Store::new(env.skillz_dir().join("library.toml"))
    }

    /// The saved catalog; empty when there is no file, [`Corrupt`] when it cannot be read.
    pub fn load(&self) -> anyhow::Result<Catalog> {
        let Some(text) = read_optional(&self.path)? else {
            return Ok(Catalog::default());
        };
        let file: File = toml_edit::de::from_str(&text).map_err(|_| Corrupt {
            path: self.path.clone(),
        })?;
        Ok(Catalog::from_items(file.items))
    }

    pub fn save(&self, catalog: &Catalog) -> anyhow::Result<()> {
        let file = File {
            items: catalog.items().to_vec(),
        };
        let text = toml_edit::ser::to_string_pretty(&file).context("writing library")?;
        write_atomic(&self.path, &text)
    }
}

#[cfg(test)]
mod tests {
    use super::super::item::tests::{hook, skill};
    use super::*;

    fn store_in(dir: &tempfile::TempDir) -> Store {
        Store::new(dir.path().join("skillz/library.toml"))
    }

    #[test]
    fn the_library_lives_beside_the_profiles_in_the_skillz_dir() {
        let env = Env::new("/home/me".into(), "/work".into());

        let store = Store::in_env(&env);

        assert_eq!(store.path, env.skillz_dir().join("library.toml"));
    }

    #[test]
    fn a_missing_file_is_an_empty_library() {
        let dir = tempfile::tempdir().unwrap();

        let catalog = store_in(&dir).load().unwrap();

        assert_eq!(catalog, Catalog::default());
    }

    #[test]
    fn a_saved_catalog_loads_back_the_same() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let catalog = Catalog::from_items(vec![skill("o/r", "a"), hook("x")]);

        store.save(&catalog).unwrap();

        assert_eq!(store.load().unwrap(), catalog);
    }

    #[test]
    fn a_corrupt_file_is_a_typed_error_and_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let file = dir.path().join("skillz/library.toml");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "item = 3 [").unwrap();

        let error = store.load().unwrap_err();

        // Assert the error is Corrupt and the bytes are still there.
        assert!(error.downcast_ref::<Corrupt>().is_some());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "item = 3 [");
    }
}
