//! The one global file skillz keeps: `store.json`, holding the library of available items with
//! where they came from, and the named profiles. A project's own state is its native tool
//! config, never kept here; only the name of the profile last applied to it is.

mod document;
mod legacy;
mod lock;

use std::path::PathBuf;

use anyhow::Context;

use crate::harness::action::stage;
use crate::harness::env::Env;
use crate::harness::read_optional::read_optional;
pub use document::Document;
use lock::Lock;

/// The store file exists but is not a valid store; it is left untouched.
#[derive(Debug, PartialEq, Eq)]
pub struct Corrupt {
    pub path: PathBuf,
}

impl std::fmt::Display for Corrupt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} is not a valid skillz store", self.path.display())
    }
}

impl std::error::Error for Corrupt {}

/// The store file at an injected path.
#[derive(Debug, Clone)]
pub struct Store {
    path: PathBuf,
}

impl Store {
    pub fn new(path: PathBuf) -> Self {
        Store { path }
    }

    pub fn in_env(env: &Env) -> Self {
        Store::new(crate::app_paths::store_path(env))
    }

    /// The stored document; empty when there is no file, [`Corrupt`] when it cannot be read.
    pub fn read(&self) -> anyhow::Result<Document> {
        self.parse(read_optional(&self.path)?.as_deref())
    }

    fn parse(&self, text: Option<&str>) -> anyhow::Result<Document> {
        let Some(text) = text else {
            return Ok(Document::default());
        };
        let corrupt = || Corrupt {
            path: self.path.clone(),
        };
        let mut raw: serde_json::Value = serde_json::from_str(text).map_err(|_| corrupt())?;
        legacy::drop_cursor(&mut raw);
        let document: Document = serde_json::from_value(raw).map_err(|_| corrupt())?;
        anyhow::ensure!(document.version == document::VERSION, corrupt());
        Ok(document)
    }

    /// Read, change and write back under a lock, so two skillz processes never lose each
    /// other's changes. Nothing is written when reading or `change` fails.
    pub fn update<T>(
        &self,
        change: impl FnOnce(&mut Document) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let _held = Lock::acquire(&self.path.with_extension("json.lock"))?;
        let original = read_optional(&self.path)?;
        let mut document = self.parse(original.as_deref())?;
        let result = change(&mut document)?;
        let text = serde_json::to_string_pretty(&document).context("writing store")?;
        let staged = stage::stage(&self.path, original, &text)?;
        stage::commit(&[staged])?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests;
