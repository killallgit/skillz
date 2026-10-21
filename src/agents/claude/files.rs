//! Where Claude keeps its plugin records and settings, and how to read them.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde_json::Value;

use crate::env::Env;
use crate::model::Scope;

pub(super) const ENABLED_PLUGINS: &str = "enabledPlugins";
pub(super) const PLUGINS: &str = "plugins";
pub(super) const EXTRA_KNOWN_MARKETPLACES: &str = "extraKnownMarketplaces";
pub(super) const INSTALL_LOCATION: &str = "installLocation";

pub(super) fn installed_file(claude: &Path) -> PathBuf {
    claude.join("plugins/installed_plugins.json")
}

pub(super) fn marketplaces_file(claude: &Path) -> PathBuf {
    claude.join("plugins/known_marketplaces.json")
}

/// Where Claude loads subagent definitions from.
pub(super) fn agents_dir(env: &Env, scope: Scope) -> PathBuf {
    match scope {
        Scope::Global => env.claude_dir().join("agents"),
        Scope::Project => env.project.join(".claude/agents"),
    }
}

/// Where a profile's hooks go: the global settings, or the project's personal (uncommitted) ones.
pub(super) fn hook_settings(env: &Env, scope: Scope) -> PathBuf {
    match scope {
        Scope::Global => env.claude_dir().join("settings.json"),
        Scope::Project => env.project.join(".claude/settings.local.json"),
    }
}

pub(super) fn settings_files(env: &Env, scope: Scope) -> Vec<PathBuf> {
    match scope {
        Scope::Global => global_settings(&env.claude_dir()),
        Scope::Project => project_settings(&env.project),
    }
}

pub(super) fn global_settings(claude: &Path) -> Vec<PathBuf> {
    vec![claude.join("settings.json")]
}

pub(super) fn project_settings(root: &Path) -> Vec<PathBuf> {
    let dir = root.join(".claude");
    vec![dir.join("settings.json"), dir.join("settings.local.json")]
}

/// Missing file = `Null` (no entries); malformed JSON is an error.
pub(super) fn read_json(path: &Path) -> anyhow::Result<Value> {
    match fs::read_to_string(path) {
        Ok(text) => {
            serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
        }
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(Value::Null),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn unreadable_path_is_err() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("x.json")).unwrap();

        let result = read_json(&tmp.path().join("x.json"));

        assert!(result.is_err());
    }
}
