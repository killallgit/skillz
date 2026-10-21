//! What each profile entry added to a scope, remembered between runs so it can be removed
//! exactly.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::atomic::write_atomic;
use crate::env::Env;
use crate::model::{Augmentation, Kind, Scope};
use crate::profile::Entry;
use crate::read_optional::read_optional;
use crate::skillz_dir;

/// One item a profile added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Owned {
    pub id: String,
    pub name: String,
    pub kind: Kind,
}

impl Owned {
    pub fn of(item: &Augmentation) -> Self {
        Owned {
            id: item.id.clone(),
            name: item.name.clone(),
            kind: item.kind,
        }
    }
}

/// An entry (with its agents resolved) and the items its install added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Loaded {
    pub entry: Entry,
    pub items: Vec<Owned>,
}

/// What a profile put into one scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub profile: String,
    pub entries: Vec<Loaded>,
}

/// The records of every scope skillz has applied a profile to, in one json file.
pub struct Ledger {
    path: PathBuf,
}

impl Ledger {
    pub fn new(path: PathBuf) -> Self {
        Ledger { path }
    }

    /// `<project>/.skillz/state.json`.
    pub fn in_env(env: &Env) -> Self {
        Ledger::new(env.skillz_dir().join("state.json"))
    }

    /// Which scope a record is about: the project's path, or `global`, plus the config dirs
    /// the environment redirects, since another `CLAUDE_CONFIG_DIR` is another set of installs.
    pub fn key(env: &Env, scope: Scope) -> String {
        let mut key = match scope {
            Scope::Global => "global".to_string(),
            Scope::Project => env.project.display().to_string(),
        };
        let redirects = [
            ("CLAUDE_CONFIG_DIR", &env.claude_config),
            ("CODEX_HOME", &env.codex_home),
        ];
        for (name, dir) in redirects {
            if let Some(dir) = dir {
                key.push_str(&format!(" {name}={}", dir.display()));
            }
        }
        key
    }

    fn read(&self) -> anyhow::Result<BTreeMap<String, Record>> {
        match read_optional(&self.path)? {
            Some(text) => serde_json::from_str(&text)
                .with_context(|| format!("invalid {}", self.path.display())),
            None => Ok(BTreeMap::new()),
        }
    }

    pub fn get(&self, key: &str) -> anyhow::Result<Option<Record>> {
        Ok(self.read()?.remove(key))
    }

    /// Store `record` for `key`, or forget `key` when `record` is `None`.
    pub fn put(&self, key: &str, record: Option<Record>) -> anyhow::Result<()> {
        let _lock = self.lock()?;
        let mut all = self.read()?;
        match record {
            Some(r) => all.insert(key.into(), r),
            None => all.remove(key),
        };
        write_atomic(&self.path, &serde_json::to_string_pretty(&all)?)
    }

    /// Held for a read-modify-write, so two skillz runs do not overwrite each other's scopes.
    fn lock(&self) -> anyhow::Result<fs::File> {
        if let Some(dir) = self.path.parent() {
            skillz_dir::ensure(dir)?;
        }
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.path.with_extension("lock"))?;
        file.lock()?;
        Ok(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ledger() -> (tempfile::TempDir, Ledger) {
        let dir = tempfile::tempdir().unwrap();
        let ledger = Ledger::new(dir.path().join("state.json"));
        (dir, ledger)
    }

    fn record(profile: &str) -> Record {
        Record {
            profile: profile.into(),
            entries: vec![],
        }
    }

    #[test]
    fn records_of_different_scopes_are_kept_apart() {
        let (_dir, ledger) = ledger();
        ledger.put("global", Some(record("a"))).unwrap();
        ledger.put("/proj", Some(record("b"))).unwrap();

        ledger.put("global", None).unwrap();

        assert!(ledger.get("global").unwrap().is_none());
        assert_eq!(ledger.get("/proj").unwrap(), Some(record("b")));
    }

    #[test]
    fn concurrent_puts_do_not_lose_each_others_scopes() {
        let (_dir, ledger) = ledger();
        let ledger = std::sync::Arc::new(ledger);

        let workers: Vec<_> = (0..8)
            .map(|i| {
                let ledger = ledger.clone();
                std::thread::spawn(move || {
                    ledger.put(&format!("scope{i}"), Some(record("p"))).unwrap();
                })
            })
            .collect();
        workers.into_iter().for_each(|w| w.join().unwrap());

        // every scope's record survived the interleaved read-modify-writes
        assert_eq!(ledger.read().unwrap().len(), 8);
    }

    #[test]
    fn a_corrupt_file_is_an_error_not_an_empty_ledger() {
        let (_dir, ledger) = ledger();
        fs::write(&ledger.path, "{nope").unwrap();

        assert!(ledger.get("x").is_err());
    }

    #[test]
    fn a_redirected_config_dir_is_its_own_scope() {
        let mut env = Env::new("/h".into(), "/p".into());
        let plain = Ledger::key(&env, Scope::Project);
        env.claude_config = Some("/c".into());
        let claude = Ledger::key(&env, Scope::Project);
        env.codex_home = Some("/x".into());
        let both = Ledger::key(&env, Scope::Global);

        assert_ne!(plain, claude);
        assert_ne!(claude, both);
        assert!(plain.starts_with("/p") && both.starts_with("global"));
    }
}
