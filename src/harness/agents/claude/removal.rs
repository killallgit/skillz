//! The actions that remove one Claude plugin install.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::files::{
    ENABLED_PLUGINS, PLUGINS, global_settings, installed_file, project_settings, read_json,
};
use super::plugins::dir_entries;
use super::records::Install;
use crate::harness::action::{Action, Edit, RemovalTarget};
use crate::harness::model::split_plugin_id;

pub(super) fn plugin_removal(claude: &Path, install: &Install) -> Vec<Action> {
    let settings = match &install.project {
        None => global_settings(claude),
        Some(p) => project_settings(p),
    };
    // `claude plugin uninstall` also drops the cache once no other scope uses the plugin.
    let cwd = install.project.as_deref().unwrap_or(claude);
    let args = [
        "plugin",
        "uninstall",
        &install.id,
        "--scope",
        &install.scope,
    ];
    let native = Action::native("claude", &args, &[&install.id], cwd);
    native
        .into_iter()
        .chain(settings.into_iter().map(|file| Action::Edit {
            file,
            edit: Edit::JsonRemoveKey {
                path: vec![ENABLED_PLUGINS.into(), install.id.clone()],
            },
        }))
        .chain(std::iter::once(Action::Edit {
            file: installed_file(claude),
            edit: Edit::JsonRemoveArrayItems {
                path: vec![PLUGINS.into(), install.id.clone()],
                fields: record_fields(install),
            },
        }))
        .chain(plugin_dirs(claude, &install.id))
        .chain(leftovers(claude, &install.id))
        .collect()
}

/// Drops repeated actions, keeping the first of each in order.
pub(super) fn unique_in_order(actions: impl IntoIterator<Item = Action>) -> Vec<Action> {
    let mut unique: Vec<Action> = Vec::new();
    for action in actions {
        if !unique.contains(&action) {
            unique.push(action);
        }
    }
    unique
}

/// Selects the `plugins[id]` records that removing `install` drops: user scope is the global
/// install; any other scope is keyed by its project.
fn record_fields(install: &Install) -> Vec<(String, Value)> {
    match &install.project {
        None => vec![("scope".into(), "user".into())],
        Some(p) => vec![("projectPath".into(), p.to_string_lossy().into())],
    }
}

fn unused_record(claude: &Path, id: &str, target: RemovalTarget) -> Action {
    Action::RemovePathIfJsonAbsent {
        target,
        file: installed_file(claude),
        key: vec![PLUGINS.into(), id.into()],
    }
}

/// The plugin's cache and data dirs, removed once `plugins[id]` is gone after the edits, so
/// whichever selected items drop its last install record take them; none unless both are
/// valid targets.
fn plugin_dirs(claude: &Path, id: &str) -> Vec<Action> {
    let (name, mp) = split_plugin_id(id);
    if mp.is_empty() {
        return vec![];
    }
    let dir = claude.join("plugins");
    let cache = RemovalTarget::new(&dir.join("cache"), [mp, name]);
    let data = RemovalTarget::new(&dir.join("data"), [format!("{name}-{mp}")]);
    cache.zip(data).map_or(vec![], |(c, d)| {
        vec![unused_record(claude, id, c), unused_record(claude, id, d)]
    })
}

/// What a plugin's leftover files are matched against.
struct Leftovers<'a> {
    claude: &'a Path,
    id: &'a str,
    name: &'a str,
    rivals: Vec<String>,
}

impl Leftovers<'_> {
    fn action_for(&self, dir: &Path, path: &Path) -> Option<Action> {
        let file = path.file_name()?.to_str()?;
        let mine = claims(self.name, file) && !self.rivals.iter().any(|r| claims(r, file));
        let target = mine.then(|| RemovalTarget::new(dir, [file]))??;
        Some(unused_record(self.claude, self.id, target))
    }
}

/// Whether `file` is named for `owner`: the owner, then end or `-`, `_`, `.`.
fn claims(owner: &str, file: &str) -> bool {
    let stem = file.trim_start_matches('.');
    stem.strip_prefix(owner)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(['-', '_', '.']))
}

/// Names of installed plugins whose longer name starts with `name`.
fn rivals(claude: &Path, name: &str) -> Vec<String> {
    read_json(&installed_file(claude))
        .ok()
        .and_then(|j| j.get(PLUGINS).and_then(Value::as_object).cloned())
        .map(|m| {
            m.keys()
                .map(|k| split_plugin_id(k).0.to_string())
                .filter(|n| n.len() > name.len() && n.starts_with(name))
                .collect()
        })
        .unwrap_or_default()
}

/// Files a plugin's own hooks wrote beside Claude's config (`~/.claude/.<name>-active`,
/// `<name>-statusline.sh`, `plugins/store/<name>_<mp>-….json`) and that no uninstall knows
/// about. Matched by name: the plugin name, then `-`, `_` or `.`. Gone with the plugin, and
/// skipped when another installed plugin's longer name claims the file (`killall` vs
/// `killall-agents`). Global installs only; names under 4 chars are too generic to match.
fn leftovers(claude: &Path, id: &str) -> Vec<Action> {
    let (name, mp) = split_plugin_id(id);
    if mp.is_empty() || name.len() < 4 {
        return vec![];
    }
    let matcher = Leftovers {
        claude,
        id,
        name,
        rivals: rivals(claude, name),
    };
    let dirs: [PathBuf; 2] = [claude.to_path_buf(), claude.join("plugins/store")];
    let mut found: Vec<Action> = dirs
        .iter()
        .flat_map(|dir| {
            dir_entries(dir)
                .into_iter()
                .filter_map(|path| matcher.action_for(dir, &path))
        })
        .collect();
    found.sort_by_key(|a| format!("{a:?}"));
    found
}

#[cfg(test)]
mod tests {
    use super::super::Claude;
    use super::super::records::installs_from;
    use super::super::testutil::*;
    use super::*;
    use crate::harness::action::apply_edit;
    use crate::harness::agents::Harness;
    use crate::harness::model::{Kind, Scope};
    use serde_json::json;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn plugin_dirs_go_once_the_last_selected_scope_drops_the_id() {
        let run = |scopes: &[Scope]| {
            let f = fixture();
            let data = f.env.home.join(".claude/plugins/data/tools-acme");
            fs::create_dir_all(&data).unwrap();
            let plan: Vec<Action> = scopes
                .iter()
                .flat_map(|&s| Claude.installed(&f.env, s).unwrap())
                .filter(|a| a.kind == Kind::Plugin && a.name == "tools@acme")
                .flat_map(|a| a.removal)
                .collect();
            crate::harness::action::execute(&plan, &crate::harness::runner::NoCommands).unwrap();
            let installed = read_json(&installed_file(&crate::harness::agents::claude::dir(
                &f.env,
            )))
            .unwrap();
            let kept = installed["plugins"].get("tools@acme").is_some();
            (kept, f.cache.exists(), data.exists())
        };
        assert_eq!(run(&[Scope::Global]), (true, true, true));
        assert_eq!(run(&[Scope::Project]), (true, true, true));
        assert_eq!(run(&[Scope::Global, Scope::Project]), (false, false, false));
    }

    #[test]
    fn install_record_edit_drops_one_scope() {
        let doc = json!({"version": 2, "plugins": {
            "p@m": [
                {"scope": "user", "installPath": "/c"},
                {"scope": "project", "projectPath": "/proj"},
                {"scope": "local", "projectPath": "/other"}
            ],
            "q@m": [{"scope": "user"}]
        }});
        let installs = installs_from(&doc);
        let after = |id: &str, project: Option<&str>| {
            let install = installs
                .iter()
                .find(|i| i.id == id && i.project.as_deref() == project.map(Path::new))
                .unwrap();
            let record_edit = plugin_removal(Path::new("/h/.claude"), install)
                .into_iter()
                .find(|a| matches!(a, Action::Edit { file, .. } if file.ends_with("installed_plugins.json")));
            let Some(Action::Edit { file, edit }) = record_edit else {
                panic!("no install record edit");
            };
            assert_eq!(file, installed_file(Path::new("/h/.claude")));
            let out = apply_edit(&file, &edit, &doc.to_string()).unwrap();
            serde_json::from_str::<Value>(&out).unwrap()["plugins"].clone()
        };
        assert_eq!(
            after("p@m", None)["p@m"],
            json!([
                {"scope": "project", "projectPath": "/proj"},
                {"scope": "local", "projectPath": "/other"}
            ])
        );
        assert_eq!(
            after("p@m", Some("/proj"))["p@m"],
            json!([
                {"scope": "user", "installPath": "/c"},
                {"scope": "local", "projectPath": "/other"}
            ])
        );
        let last = after("q@m", None);
        assert!(last.get("q@m").is_none());
        assert!(last.get("p@m").is_some());
    }

    #[test]
    fn plugin_dirs_need_both_halves_of_the_id() {
        // Segment rules are `RemovalTarget`'s; here the data dir must not outlive a bad cache dir.
        for id in ["bare", "@acme", "x@", "x@.."] {
            assert_eq!(plugin_dirs(Path::new("/h/.claude"), id), [], "{id}");
        }
        assert_eq!(plugin_dirs(Path::new("/h/.claude"), "a@b@acme").len(), 2);
    }

    #[test]
    fn plugins_remove_with_claude_first() {
        let home = Path::new("/h");
        let user = Install {
            id: "p@mp".into(),
            project: None,
            scope: "user".into(),
            install_path: None,
            version: None,
        };
        let local = Install {
            project: Some(PathBuf::from("/proj")),
            scope: "local".into(),
            ..user.clone()
        };
        let first = |i: &Install| {
            plugin_removal(&home.join(".claude"), i)
                .into_iter()
                .next()
                .unwrap()
        };
        let uninstall = |scope: &str, cwd: &str| Action::Run {
            program: "claude".into(),
            args: ["plugin", "uninstall", "p@mp", "--scope", scope]
                .map(String::from)
                .to_vec(),
            cwd: cwd.into(),
            fallback: true,
        };
        assert_eq!(first(&user), uninstall("user", "/h/.claude"));
        assert_eq!(first(&local), uninstall("local", "/proj"));
        let flag = Install {
            id: "-x@mp".into(),
            ..user.clone()
        };
        assert!(
            !plugin_removal(&home.join(".claude"), &flag)
                .iter()
                .any(|a| matches!(a, Action::Run { .. }))
        );
    }

    #[test]
    fn unique_in_order_keeps_first_occurrences() {
        let a = rm(Path::new("/r"), &["a"]);
        let b = rm(Path::new("/r"), &["b"]);

        let unique = unique_in_order([a.clone(), b.clone(), a.clone()]);

        assert_eq!(unique, [a, b]);
    }

    #[test]
    fn leftovers_match_plugin_named_files_but_not_a_longer_plugin_s() {
        let home = TempDir::new().unwrap();
        let claude = home.path().join(".claude");
        fs::create_dir_all(claude.join("plugins/store")).unwrap();
        for f in [
            ".killall-active",
            "killall-modes",
            "killall-agents-state",
            "killallx",
            "plugins/store/killall_mp-1.json",
        ] {
            fs::write(claude.join(f), "").unwrap();
        }
        fs::write(
            claude.join("plugins/installed_plugins.json"),
            json!({"plugins": {"killall@mp": [], "killall-agents@mp": []}}).to_string(),
        )
        .unwrap();

        let names: Vec<String> = leftovers(&home.path().join(".claude"), "killall@mp")
            .iter()
            .map(|a| format!("{a:?}"))
            .collect();

        let has = |f: &str| names.iter().any(|n| n.contains(f));
        assert_eq!(names.len(), 3);
        assert!(has(".killall-active") && has("killall-modes") && has("killall_mp-1.json"));
        assert!(leftovers(&home.path().join(".claude"), "ab@mp").is_empty());
    }
}
