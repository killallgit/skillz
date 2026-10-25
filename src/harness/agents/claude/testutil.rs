//! Fixtures and plan helpers shared by the Claude adapter's tests.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;
use tempfile::TempDir;

use super::files::{installed_file, marketplaces_file};
use crate::harness::action::{Action, Edit, RemovalTarget};
use crate::harness::env::Env;
use crate::harness::model::Installed;

pub(super) struct Fixture {
    _tmp: TempDir,
    pub(super) env: Env,
    pub(super) cache: PathBuf,
}

/// A removal plan without its native-tool commands.
pub(super) fn files(plan: &[Action]) -> Vec<Action> {
    plan.iter()
        .filter(|a| !matches!(a, Action::Run { .. }))
        .cloned()
        .collect()
}

pub(super) fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

pub(super) fn edit(file: &Path, edit: Edit) -> Action {
    Action::Edit {
        file: file.to_path_buf(),
        edit,
    }
}

pub(super) fn rm(root: &Path, segments: &[&str]) -> Action {
    Action::RemovePath(RemovalTarget::new(root, segments).unwrap())
}

pub(super) fn remove_key(file: &Path, path: &[&str]) -> Action {
    edit(
        file,
        Edit::JsonRemoveKey {
            path: path.iter().map(|s| s.to_string()).collect(),
        },
    )
}

/// `name@mp`'s cache and data dir removals, each waiting for `plugins[id]` to be gone.
pub(super) fn dirs(home: &Path, name: &str, mp: &str) -> Vec<Action> {
    let plugins = home.join(".claude/plugins");
    let unused = |root: &str, segments: &[&str]| Action::RemovePathIfJsonAbsent {
        target: RemovalTarget::new(&plugins.join(root), segments).unwrap(),
        file: installed_file(&home.join(".claude")),
        key: vec!["plugins".into(), format!("{name}@{mp}")],
    };
    vec![
        unused("cache", &[mp, name]),
        unused("data", &[&format!("{name}-{mp}")]),
    ]
}

pub(super) fn remove_install(file: &Path, id: &str, project: Option<&Path>) -> Action {
    edit(
        file,
        Edit::JsonRemoveArrayItems {
            path: vec!["plugins".into(), id.into()],
            fields: vec![match project {
                None => ("scope".into(), json!("user")),
                Some(p) => ("projectPath".into(), json!(p)),
            }],
        },
    )
}

/// `tools@acme` installed user-scope and in the project (twice: project + local),
/// `solo@acme` only in another project, `lint@other` user-scope from a url marketplace.
pub(super) fn fixture() -> Fixture {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let project = tmp.path().join("proj");
    let other = tmp.path().join("other");
    fs::create_dir_all(&project).unwrap();
    let cache = home.join(".claude/plugins/cache/acme/tools/1.0.0");
    write(&cache.join("skills/alpha/SKILL.md"), "# a");
    write(&cache.join("skills/beta/SKILL.md"), "# b");
    fs::create_dir_all(cache.join("skills/not-a-skill")).unwrap();
    write(&cache.join("agents/reviewer.md"), "x");
    write(&cache.join("agents/notes.txt"), "x");
    write(&cache.join("commands/ship.md"), "x");
    write(
        &cache.join("hooks/hooks.json"),
        r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"say done"}]}]}}"#,
    );
    let mp_dir = home.join(".claude/plugins/marketplaces/acme");
    let installed = json!({
        "version": 2,
        "plugins": {
            "tools@acme": [
                {"scope": "user", "installPath": cache, "version": "1.0.0"},
                {"scope": "project", "projectPath": project, "installPath": cache, "version": "1.0.0"},
                {"scope": "local", "projectPath": project, "installPath": cache, "version": "1.0.0"}
            ],
            "solo@acme": [
                {"scope": "project", "projectPath": other, "installPath": "/nonexistent/solo"}
            ],
            "lint@other": [
                {"scope": "user", "installPath": "/nonexistent/lint"},
                {"scope": "project"}
            ]
        }
    });
    let known = json!({
        "acme": {"source": {"source": "github", "repo": "acme/plugins"}, "installLocation": mp_dir},
        "other": {"source": {"source": "git", "url": "https://example.com/other.git"}, "installLocation": ""}
    });
    write(
        &installed_file(&home.join(".claude")),
        &installed.to_string(),
    );
    write(
        &marketplaces_file(&home.join(".claude")),
        &known.to_string(),
    );
    Fixture {
        _tmp: tmp,
        env: Env {
            home,
            project,
            claude_config: None,
            codex_home: None,
        },
        cache,
    }
}

pub(super) fn by_id<'a>(items: &'a [Installed], id: &str) -> &'a Installed {
    items.iter().find(|a| a.id == id).unwrap()
}
