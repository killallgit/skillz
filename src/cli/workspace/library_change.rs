//! Changing the library: adding a typed entry, importing what is installed, removing an item.

use super::Session;
use super::notice::Notice;
use crate::cli::recording::now_secs;
use crate::cli::scope::{installed, project_env};
use crate::library::{LibraryItem, import_installed};
use crate::model::Scope;
use crate::profile::Entry;

impl Session<'_, '_> {
    /// Items typed in carry no provenance; one the library already knows keeps its own.
    pub(super) fn library_add(&self, spec: &str) -> anyhow::Result<Option<Notice>> {
        let entry = Entry::parse_spec(spec)?;
        let label = entry.to_string();
        self.store.update(|document| {
            document.items.add_entry(entry);
            Ok(())
        })?;
        Ok(Some(Notice::LibraryAdded(label)))
    }

    pub(super) fn library_import(&self) -> anyhow::Result<Option<Notice>> {
        let env = project_env(self.deps, &self.project);
        let items = installed(self.deps, &env, Scope::Project);
        let (added, skipped) = self.store.update(|document| {
            let before = document.items.items().len();
            let skipped = import_installed(&mut document.items, &items, now_secs());
            Ok((document.items.items().len() - before, skipped))
        })?;
        Ok(Some(Notice::Imported { added, skipped }))
    }

    pub(super) fn library_remove(&self, entry: &Entry) -> anyhow::Result<Option<Notice>> {
        let item = LibraryItem::from_entry(entry.clone());
        self.store.update(|document| {
            document.items.remove(&item);
            Ok(())
        })?;
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
    use crate::{profile, store};

    fn with_session<T>(dir: &Path, test: impl FnOnce(&Session) -> T) -> T {
        let runner = FakeRunner::new(|_| {});
        let prompter = ScriptedPrompter::new(&[], true);
        let deps = Deps::for_test(dir, registry(), (&runner, &prompter));
        let session = Session {
            deps: &deps,
            profiles: profile::Store::new(dir.join("store.json")),
            store: store::Store::new(dir.join("store.json")),
            project: dir.to_path_buf(),
        };
        test(&session)
    }

    #[test]
    fn a_typed_line_is_saved_and_removed_again() {
        let dir = tempfile::tempdir().unwrap();

        let (added, removed) = with_session(dir.path(), |session| {
            session.library_add("skill o/r tdd").unwrap();
            let added = session.store.read().unwrap().items.items().len();
            let entry = Entry::parse_spec("skill o/r tdd").unwrap();
            session.library_remove(&entry).unwrap();
            (added, session.store.read().unwrap().items.items().len())
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
            session.store.read().unwrap().items.items().len()
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
