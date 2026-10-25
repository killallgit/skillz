use std::path::{Path, PathBuf};

use crate::model::Scope;

/// Filesystem roots every flow reads from. Injected so tests can point at temp dirs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Env {
    pub home: PathBuf,
    pub project: PathBuf,
    /// `CLAUDE_CONFIG_DIR`: where Claude keeps its global config, instead of `~/.claude`.
    pub claude_config: Option<PathBuf>,
    /// `CODEX_HOME`: where Codex keeps its global config, instead of `~/.codex`.
    pub codex_home: Option<PathBuf>,
}

impl Env {
    pub fn new(home: PathBuf, project: PathBuf) -> Self {
        Env {
            home,
            project,
            claude_config: None,
            codex_home: None,
        }
    }

    pub fn root(&self, scope: Scope) -> &Path {
        match scope {
            Scope::Global => &self.home,
            Scope::Project => &self.project,
        }
    }
}
