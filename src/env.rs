use std::path::{Path, PathBuf};

use crate::model::Scope;

/// Filesystem roots every flow reads from. Injected so tests can point at temp dirs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Env {
    pub home: PathBuf,
    pub project: PathBuf,
}

impl Env {
    pub fn root(&self, scope: Scope) -> &Path {
        match scope {
            Scope::Global => &self.home,
            Scope::Project => &self.project,
        }
    }
}
