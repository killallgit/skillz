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

    /// Claude's global config dir: `CLAUDE_CONFIG_DIR` when set, else `~/.claude`.
    pub fn claude_dir(&self) -> PathBuf {
        self.claude_config
            .clone()
            .unwrap_or_else(|| self.home.join(".claude"))
    }

    /// Codex's global config dir: `CODEX_HOME` when set, else `~/.codex`.
    pub fn codex_dir(&self) -> PathBuf {
        self.codex_home
            .clone()
            .unwrap_or_else(|| self.home.join(".codex"))
    }

    /// Codex config dir for a scope: the global one, or the project's `.codex`.
    pub fn codex_scope_dir(&self, scope: Scope) -> PathBuf {
        match scope {
            Scope::Global => self.codex_dir(),
            Scope::Project => self.project.join(".codex"),
        }
    }

    /// Claude's global state file: inside `CLAUDE_CONFIG_DIR` when set, else `~/.claude.json`.
    pub fn claude_json(&self) -> PathBuf {
        match &self.claude_config {
            Some(dir) => dir.join(".claude.json"),
            None => self.home.join(".claude.json"),
        }
    }

    /// skillz's own config: profiles and the workspace registry.
    pub fn skillz_dir(&self) -> PathBuf {
        self.home.join(".config/skillz")
    }

    pub fn root(&self, scope: Scope) -> &Path {
        match scope {
            Scope::Global => &self.home,
            Scope::Project => &self.project,
        }
    }
}
