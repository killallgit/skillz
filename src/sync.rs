//! Keeping the library, the master record of everything seen, in step with what is installed.
//! Syncing adds discoveries and follows unambiguous moves of local sources.

use std::sync::Arc;
use std::thread;

use crate::agents::{inventory, registry};
use crate::env::Env;
use crate::library::{Catalog, Synced, now_secs, repoint_profiles, sync_installed};
use crate::model::{Augmentation, Scope};
use crate::stale::{Disk, Probe};
use crate::store::{Document, Store};

/// Finds everything installed for a project and at user level.
pub trait Scanner: Send + Sync {
    fn scan(&self, env: &Env) -> anyhow::Result<Vec<Augmentation>>;
}

/// Reads the agents' own config for every adapter.
pub struct InstalledScanner;

impl Scanner for InstalledScanner {
    fn scan(&self, env: &Env) -> anyhow::Result<Vec<Augmentation>> {
        inventory(&registry(), env, &[Scope::Global, Scope::Project]).complete()
    }
}

/// What a sync did, with the library as it stands afterwards.
#[derive(Debug, Clone)]
pub struct Report {
    pub synced: Synced,
    pub library: Catalog,
}

/// Add what `items` hold that the document's library lacks, and follow moved folders in the
/// library and the profiles (see `library::sync_installed`).
pub fn merge(document: &mut Document, items: &[Augmentation], probe: &dyn Probe) -> Synced {
    let mut synced = sync_installed(&mut document.items, items, (now_secs(), probe));
    for moved in &mut synced.repointed {
        moved.profiles = repoint_profiles(&mut document.profiles, moved);
    }
    synced
}

/// Merge `items` into the stored library. A dry run reports the same without writing. The
/// store is locked only for the merge.
pub fn sync_library(
    store: &Store,
    items: &[Augmentation],
    (dry_run, probe): (bool, &dyn Probe),
) -> anyhow::Result<Report> {
    if dry_run {
        let mut document = store.read()?;
        let synced = merge(&mut document, items, probe);
        return Ok(Report {
            synced,
            library: document.items,
        });
    }
    store.update(|document| {
        let synced = merge(document, items, probe);
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
        done(
            scanner
                .scan(&env)
                .and_then(|items| sync_library(&Store::in_env(&env), &items, (false, &Disk))),
        );
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
        fn scan(&self, _env: &Env) -> anyhow::Result<Vec<Augmentation>> {
            Ok(self.0.clone())
        }
    }

    fn installed_skill(name: &str) -> Augmentation {
        Augmentation {
            details: Default::default(),
            hooks: vec![],
            members: Vec::new(),
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

        let report = sync_library(&store, &[installed_skill("tdd")], (true, &Disk)).unwrap();

        assert_eq!(report.synced.added.len(), 1);
        assert_eq!(store.read().unwrap().items, Catalog::default());
    }

    #[test]
    fn a_store_that_cannot_be_read_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.json");
        std::fs::write(&path, "not json").unwrap();

        let result = sync_library(&Store::new(path), &[installed_skill("tdd")], (false, &Disk));

        assert!(result.is_err());
    }

    #[test]
    fn a_failed_scan_never_creates_a_store() {
        let dir = tempfile::tempdir().unwrap();
        let env = env_in(dir.path());
        let settings = env.home.join(".claude/settings.json");
        std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
        std::fs::write(settings, "{invalid").unwrap();
        let (tx, rx) = mpsc::channel();

        spawn(Arc::new(InstalledScanner), env.clone(), move |report| {
            tx.send(report).unwrap()
        });

        assert!(rx.recv().unwrap().is_err());
        assert!(!env.store_path().exists());
    }
}
