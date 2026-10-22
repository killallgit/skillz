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

    /// Where `codex` commands that act on the user config run: the config dir's parent.
    pub fn codex_cwd(&self) -> PathBuf {
        let dir = self.codex_dir();
        dir.parent().unwrap_or(&self.home).to_path_buf()
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

    /// skillz's own config: the store and the dirs `run` keeps.
    pub fn skillz_dir(&self) -> PathBuf {
        self.home.join(".config/skillz")
    }

    /// The one global store of the library and the profiles.
    pub fn store_path(&self) -> PathBuf {
        self.skillz_dir().join("store.json")
    }

    /// Resolve local skill and subagent sources for this project. Repository references stay
    /// portable; explicit local paths stay local even after their directory disappears.
    pub fn resolve_source(&self, source: &str) -> String {
        if source.contains("://") || source.starts_with("git@") {
            return source.to_string();
        }
        let path = self.project.join(source);
        if path.is_dir() {
            return std::fs::canonicalize(&path)
                .unwrap_or(path)
                .to_string_lossy()
                .into_owned();
        }
        if Path::new(source).is_absolute()
            || matches!(source, "." | "..")
            || source.starts_with("./")
            || source.starts_with("../")
        {
            return std::path::absolute(&path)
                .unwrap_or(path)
                .to_string_lossy()
                .into_owned();
        }
        source.to_string()
    }

    pub fn root(&self, scope: Scope) -> &Path {
        match scope {
            Scope::Global => &self.home,
            Scope::Project => &self.project,
        }
    }
}
