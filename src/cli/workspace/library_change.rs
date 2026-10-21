//! Changing the library: adding a typed entry, importing what is installed, removing an item.

use super::Session;
use super::notice::Notice;
use crate::cli::scope::{installed, project_env};
use crate::library::{LibraryItem, import_installed};
use crate::model::Scope;
use crate::profile::Entry;

impl Session<'_, '_> {
    pub(super) fn library_add(&self, spec: &str) -> anyhow::Result<Option<Notice>> {
        let entry = Entry::parse_spec(spec)?;
        let mut catalog = self.library.load()?;
        let label = entry.to_string();
        catalog.add_entry(entry);
        self.library.save(&catalog)?;
        Ok(Some(Notice::LibraryAdded(label)))
    }

    pub(super) fn library_import(&self) -> anyhow::Result<Option<Notice>> {
        let env = project_env(self.deps, &self.project);
        let items = installed(self.deps, &env, Scope::Project);
        let mut catalog = self.library.load()?;
        let before = catalog.items().len();
        let skipped = import_installed(&mut catalog, &items);
        self.library.save(&catalog)?;
        Ok(Some(Notice::Imported {
            added: catalog.items().len() - before,
            skipped,
        }))
    }

    pub(super) fn library_remove(&self, entry: &Entry) -> anyhow::Result<Option<Notice>> {
        let mut catalog = self.library.load()?;
        let item = LibraryItem::from_entry(entry.clone());
        catalog.remove(&item);
        self.library.save(&catalog)?;
        Ok(Some(Notice::LibraryRemoved(item.label)))
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::agents::registry;
    use crate::cli::deps::Deps;
    use crate::install::test_support::{FakeRunner, ScriptedPrompter};
    use crate::{library, profile};

    fn with_session<T>(dir: &Path, test: impl FnOnce(&Session) -> T) -> T {
        let runner = FakeRunner::new(|_| {});
        let prompter = ScriptedPrompter::new(&[], true);
        let deps = Deps::for_test(dir, registry(), (&runner, &prompter));
        let session = Session {
            deps: &deps,
            profiles: profile::Store::new(dir.join("profiles")),
            library: library::Store::new(dir.join("library.toml")),
            project: dir.to_path_buf(),
        };
        test(&session)
    }

    #[test]
    fn a_typed_line_is_saved_and_removed_again() {
        let dir = tempfile::tempdir().unwrap();

        let (added, removed) = with_session(dir.path(), |session| {
            session.library_add("skill o/r tdd").unwrap();
            let added = session.library.load().unwrap().items().len();
            let entry = Entry::parse_spec("skill o/r tdd").unwrap();
            session.library_remove(&entry).unwrap();
            (added, session.library.load().unwrap().items().len())
        });

        assert_eq!((added, removed), (1, 0));
    }

    #[test]
    fn a_line_that_is_malformed_or_carries_a_secret_is_an_error_and_saves_nothing() {
        let dir = tempfile::tempdir().unwrap();

        let saved = with_session(dir.path(), |session| {
            let bad = session.library_add("skill o/r");
            let secret = session.library_add("hook Stop run --token abc");
            assert!(bad.is_err() && secret.is_err());
            session.library.load().unwrap().items().len()
        });

        assert_eq!(saved, 0);
    }

    #[test]
    fn importing_an_empty_project_adds_nothing() {
        let dir = tempfile::tempdir().unwrap();

        let notice = with_session(dir.path(), |session| session.library_import().unwrap());

        assert!(matches!(
            notice,
            Some(Notice::Imported { added: 0, ref skipped }) if skipped.is_empty()
        ));
    }
}
