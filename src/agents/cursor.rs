use std::path::PathBuf;

use super::AgentAdapter;
use crate::env::Env;
use crate::model::{Agent, Augmentation, Scope};

pub struct Cursor;

impl AgentAdapter for Cursor {
    fn agent(&self) -> Agent {
        Agent::Cursor
    }

    fn npx_id(&self) -> &'static str {
        "cursor"
    }

    fn home_dir_names(&self) -> &'static [&'static str] {
        &["cursor"]
    }

    fn skill_dirs(&self, env: &Env, scope: Scope) -> Vec<PathBuf> {
        vec![env.root(scope).join(".cursor/skills")]
    }

    fn hook_files(&self, env: &Env, scope: Scope) -> Vec<PathBuf> {
        vec![env.root(scope).join(".cursor/hooks.json")]
    }

    fn discover(&self, _env: &Env, _scope: Scope) -> anyhow::Result<Vec<Augmentation>> {
        // shortcut: Cursor plugins/marketplaces are not discovered, add once Cursor's plugin storage format is mapped.
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> Env {
        Env {
            home: PathBuf::from("/h"),
            project: PathBuf::from("/p"),
            claude_config: None,
            codex_home: None,
        }
    }

    #[test]
    fn paths_per_scope() {
        let e = env();
        assert_eq!(Cursor.agent(), Agent::Cursor);
        assert_eq!(
            Cursor.skill_dirs(&e, Scope::Global),
            vec![PathBuf::from("/h/.cursor/skills")]
        );
        assert_eq!(
            Cursor.skill_dirs(&e, Scope::Project),
            vec![PathBuf::from("/p/.cursor/skills")]
        );
        assert_eq!(
            Cursor.hook_files(&e, Scope::Global),
            vec![PathBuf::from("/h/.cursor/hooks.json")]
        );
        assert_eq!(
            Cursor.hook_files(&e, Scope::Project),
            vec![PathBuf::from("/p/.cursor/hooks.json")]
        );
    }

    #[test]
    fn discover_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let e = Env {
            home: dir.path().into(),
            project: dir.path().into(),
            claude_config: None,
            codex_home: None,
        };
        assert!(Cursor.discover(&e, Scope::Global).unwrap().is_empty());
        assert!(Cursor.discover(&e, Scope::Project).unwrap().is_empty());
    }
}
