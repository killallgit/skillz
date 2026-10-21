//! Keeping the library, the master record of everything seen, in step with what is installed.
//! Syncing only adds; leaving the library is the explicit remove and nothing else.

use std::sync::Arc;
use std::thread;

use crate::agents::{inventory, registry};
use crate::env::Env;
use crate::library::{Catalog, Synced, now_secs, sync_installed};
use crate::model::{Augmentation, Scope};
use crate::store::Store;

/// Finds everything installed for a project and at user level.
pub trait Scanner: Send + Sync {
    fn scan(&self, env: &Env) -> Vec<Augmentation>;
}

/// Reads the agents' own config for every adapter.
pub struct InstalledScanner;

impl Scanner for InstalledScanner {
    fn scan(&self, env: &Env) -> Vec<Augmentation> {
        inventory(&registry(), env, &[Scope::Global, Scope::Project]).items
    }
}

/// What a sync did, with the library as it stands afterwards.
#[derive(Debug, Clone)]
pub struct Report {
    pub synced: Synced,
    pub library: Catalog,
}

/// Add what `items` hold that the stored library lacks. A dry run reports the same without
/// writing. The store is locked only for the merge.
pub fn sync_library(
    store: &Store,
    items: &[Augmentation],
    dry_run: bool,
) -> anyhow::Result<Report> {
    if dry_run {
        let mut library = store.read()?.items;
        let synced = sync_installed(&mut library, items, now_secs());
        return Ok(Report { synced, library });
    }
    store.update(|document| {
        let synced = sync_installed(&mut document.items, items, now_secs());
        Ok(Report {
            synced,
            library: document.items.clone(),
        })
    })
}

/// Scan and sync on a thread of their own, then hand the outcome to `done`.
pub fn spawn(
    scanner: Arc<dyn Scanner>,
    env: Env,
    done: impl FnOnce(anyhow::Result<Report>) + Send + 'static,
) {
    thread::spawn(move || {
        let items = scanner.scan(&env);
        done(sync_library(&Store::in_env(&env), &items, false));
    });
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;
    use crate::library::Catalog;
    use crate::model::{Agent, Installer, Kind, Origin};

    struct Fixed(Vec<Augmentation>);

    impl Scanner for Fixed {
        fn scan(&self, _env: &Env) -> Vec<Augmentation> {
            self.0.clone()
        }
    }

    fn installed_skill(name: &str) -> Augmentation {
        Augmentation {
            hooks: vec![],
            id: format!("skill:{name}"),
            name: name.into(),
            kind: Kind::Skill,
            agents: vec![Agent::Claude],
            scope: Scope::Project,
            group: "o/r".into(),
            source: Some("o/r".into()),
            installer: Installer::Npx,
            origin: Origin::default(),
            children: vec![],
            removal: vec![],
        }
    }

    fn env_in(dir: &std::path::Path) -> Env {
        Env::new(dir.to_path_buf(), dir.join("project"))
    }

    #[test]
    fn the_background_sync_hands_over_what_it_added_and_stored_it() {
        let dir = tempfile::tempdir().unwrap();
        let env = env_in(dir.path());
        let scanner = Arc::new(Fixed(vec![installed_skill("tdd")]));
        let (tx, rx) = mpsc::channel();

        spawn(scanner, env.clone(), move |report| tx.send(report).unwrap());

        // Assert the reported library and the stored one both hold the skill.
        let report = rx.recv().unwrap().unwrap();
        let stored = Store::in_env(&env).read().unwrap().items;
        assert_eq!((report.synced.added.len(), stored.items().len()), (1, 1));
        assert_eq!(report.library, stored);
    }

    #[test]
    fn a_dry_run_reports_without_writing() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().join("store.json"));

        let report = sync_library(&store, &[installed_skill("tdd")], true).unwrap();

        assert_eq!(report.synced.added.len(), 1);
        assert_eq!(store.read().unwrap().items, Catalog::default());
    }

    #[test]
    fn a_store_that_cannot_be_read_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.json");
        std::fs::write(&path, "not json").unwrap();

        let result = sync_library(&Store::new(path), &[installed_skill("tdd")], false);

        assert!(result.is_err());
    }
}
