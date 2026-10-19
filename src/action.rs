use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde_json::Value;

/// A single side effect needed to remove an augmentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Delete a file, symlink, or directory tree. Missing paths are not an error.
    RemovePath(PathBuf),
    /// Rewrite a config file in place. Missing files are skipped.
    Edit { file: PathBuf, edit: Edit },
}

/// A pure transformation of a config file's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    /// Remove every hook handler under `event` whose command equals `command`.
    /// Handles the nested layout (`hooks.<Event>[].hooks[].command`, Claude/Codex) and the
    /// flat layout (`hooks.<event>[].command`, Cursor). Prunes emptied groups and events.
    JsonRemoveHook { event: String, command: String },
    /// Remove the object key at `path` (e.g. `["enabledPlugins", "x@y"]`). No-op if absent.
    JsonRemoveKey { path: Vec<String> },
    /// Claude `installed_plugins.json`: drop entries of `plugins[id]` for one scope.
    /// `project: None` drops `"scope": "user"` entries; `Some(p)` drops entries whose
    /// `projectPath` equals `p`. Removes `plugins[id]` when it becomes empty.
    JsonRemovePluginInstall {
        id: String,
        project: Option<PathBuf>,
    },
    /// Remove the TOML key/table at `path` (e.g. `["plugins", "x@y"]`), keeping formatting.
    TomlRemoveKey { path: Vec<String> },
    /// Codex `config.toml`: remove every handler under `hooks.<event>` whose command equals
    /// `command` (`[[hooks.E]]`/`[[hooks.E.hooks]]` or inline arrays), keeping formatting.
    /// Prunes emptied groups and events. No-op if absent.
    TomlRemoveHook { event: String, command: String },
}

/// What `execute` actually changed.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    pub removed: Vec<PathBuf>,
    pub edited: Vec<PathBuf>,
}

/// Apply `edit` to `contents`. The result is guaranteed to parse in the same format.
pub fn apply_edit(edit: &Edit, contents: &str) -> anyhow::Result<String> {
    match edit {
        Edit::TomlRemoveKey { path } => toml_remove_key(contents, path),
        Edit::TomlRemoveHook { event, command } => toml_remove_hook(contents, event, command),
        Edit::JsonRemoveHook { event, command } => {
            edit_json(contents, |doc| remove_hook(doc, event, command))
        }
        Edit::JsonRemoveKey { path } => edit_json(contents, |doc| remove_json_key(doc, path)),
        Edit::JsonRemovePluginInstall { id, project } => edit_json(contents, |doc| {
            remove_plugin_install(doc, id, project.as_deref())
        }),
    }
}

/// Parse, mutate, re-serialize. An edit that changes nothing returns `contents` verbatim so
/// callers can detect no-ops by comparing text.
fn edit_json(contents: &str, mutate: impl FnOnce(&mut Value)) -> anyhow::Result<String> {
    let original: Value = serde_json::from_str(contents).context("invalid JSON")?;
    let mut doc = original.clone();
    mutate(&mut doc);
    if doc == original {
        return Ok(contents.to_string());
    }
    Ok(serde_json::to_string_pretty(&doc)? + "\n")
}

fn remove_json_key(doc: &mut Value, path: &[String]) {
    let Some((last, parents)) = path.split_last() else {
        return;
    };
    let parent = parents
        .iter()
        .try_fold(doc, |v, key| v.get_mut(key.as_str()));
    if let Some(obj) = parent.and_then(Value::as_object_mut) {
        obj.shift_remove(last);
    }
}

fn remove_hook(doc: &mut Value, event: &str, command: &str) {
    let Some(events) = doc.get_mut("hooks").and_then(Value::as_object_mut) else {
        return;
    };
    let Some(groups) = events.get_mut(event).and_then(Value::as_array_mut) else {
        return;
    };
    let before = groups.len();
    groups.retain_mut(|group| keep_hook_group(group, command));
    if groups.is_empty() && before > 0 {
        events.shift_remove(event);
    }
}

/// Nested groups (`{"hooks": [...]}`) lose matching handlers and are dropped once emptied;
/// flat entries (`{"command": ...}`) are dropped when they match.
fn keep_hook_group(group: &mut Value, command: &str) -> bool {
    match group.get_mut("hooks").and_then(Value::as_array_mut) {
        Some(handlers) => {
            let before = handlers.len();
            handlers.retain(|h| !has_command(h, command));
            !handlers.is_empty() || before == 0
        }
        None => !has_command(group, command),
    }
}

fn has_command(handler: &Value, command: &str) -> bool {
    handler.get("command").and_then(Value::as_str) == Some(command)
}

fn remove_plugin_install(doc: &mut Value, id: &str, project: Option<&Path>) {
    let Some(plugins) = doc.get_mut("plugins").and_then(Value::as_object_mut) else {
        return;
    };
    let Some(installs) = plugins.get_mut(id).and_then(Value::as_array_mut) else {
        return;
    };
    let before = installs.len();
    installs.retain(|install| !install_matches(install, project));
    if installs.is_empty() && before > 0 {
        plugins.shift_remove(id);
    }
}

fn install_matches(install: &Value, project: Option<&Path>) -> bool {
    match project {
        None => install.get("scope").and_then(Value::as_str) == Some("user"),
        Some(p) => {
            install
                .get("projectPath")
                .and_then(Value::as_str)
                .map(Path::new)
                == Some(p)
        }
    }
}

fn toml_remove_key(contents: &str, path: &[String]) -> anyhow::Result<String> {
    let mut doc: toml_edit::DocumentMut = contents.parse().context("invalid TOML")?;
    let Some((last, parents)) = path.split_last() else {
        return Ok(contents.to_string());
    };
    let parent = parents
        .iter()
        .try_fold(doc.as_item_mut(), |item, key| item.get_mut(key.as_str()));
    let removed = parent
        .and_then(toml_edit::Item::as_table_like_mut)
        .and_then(|table| table.remove(last));
    Ok(match removed {
        Some(_) => doc.to_string(),
        None => contents.to_string(),
    })
}

fn toml_remove_hook(contents: &str, event: &str, command: &str) -> anyhow::Result<String> {
    let mut doc: toml_edit::DocumentMut = contents.parse().context("invalid TOML")?;
    let Some(events) = doc
        .get_mut("hooks")
        .and_then(toml_edit::Item::as_table_like_mut)
    else {
        return Ok(contents.to_string());
    };
    let Some(groups) = events.get_mut(event) else {
        return Ok(contents.to_string());
    };
    let mut changed = false;
    let (before, after) = retain_tables(groups, |group| {
        let Some(handlers) = group.get_mut("hooks") else {
            return true;
        };
        let (b, a) = retain_tables(handlers, |h| {
            h.get("command").and_then(toml_edit::Item::as_str) != Some(command)
        });
        changed |= a != b;
        a > 0 || b == 0
    });
    if after == 0 && before > 0 {
        events.remove(event);
    }
    Ok(if changed {
        doc.to_string()
    } else {
        contents.to_string()
    })
}

/// Keep the tables of `[[x]]` or `x = [{..}, ..]` that `keep` approves (it may also edit
/// them); other values stay. Returns the table count before and after.
fn retain_tables(
    item: &mut toml_edit::Item,
    mut keep: impl FnMut(&mut dyn toml_edit::TableLike) -> bool,
) -> (usize, usize) {
    let keeps: Vec<bool> = match item {
        toml_edit::Item::ArrayOfTables(a) => a.iter_mut().map(|t| keep(t)).collect(),
        toml_edit::Item::Value(toml_edit::Value::Array(a)) => a
            .iter_mut()
            .filter_map(toml_edit::Value::as_inline_table_mut)
            .map(|t| keep(t))
            .collect(),
        _ => return (0, 0),
    };
    let mut next = keeps.iter().copied();
    match item {
        toml_edit::Item::ArrayOfTables(a) => a.retain(|_| next.next().unwrap_or(true)),
        toml_edit::Item::Value(toml_edit::Value::Array(a)) => {
            a.retain(|v| !v.is_inline_table() || next.next().unwrap_or(true))
        }
        _ => {}
    }
    (keeps.len(), keeps.iter().filter(|k| **k).count())
}

/// Dedupe, then plan every edit in memory (any failure returns before the filesystem is
/// touched), then write all edited files or none, then remove paths in their original order.
pub fn execute(actions: &[Action]) -> anyhow::Result<Report> {
    let mut unique: Vec<&Action> = Vec::new();
    for action in actions {
        if !unique.contains(&action) {
            unique.push(action);
        }
    }

    // Per file: `Some((original, planned))`, or `None` when the file is missing.
    let mut plans: Vec<(&PathBuf, Option<(String, String)>)> = Vec::new();
    for action in &unique {
        let Action::Edit { file, edit } = action else {
            continue;
        };
        let i = match plans.iter().position(|(f, _)| *f == file) {
            Some(i) => i,
            None => {
                plans.push((file, read_existing(file)?.map(|s| (s.clone(), s))));
                plans.len() - 1
            }
        };
        let Some((_, current)) = &mut plans[i].1 else {
            continue;
        };
        let after =
            apply_edit(edit, current).with_context(|| format!("edit {}", file.display()))?;
        if after != *current {
            validate(file, &after).with_context(|| format!("validate {}", file.display()))?;
            *current = after;
        }
    }

    let mut report = Report::default();
    let mut staged = Vec::new();
    for (file, plan) in plans {
        if let Some((before, after)) = plan
            && before != after
        {
            match stage(file, before, &after) {
                Ok(s) => staged.push(s),
                Err(e) => {
                    discard(&staged);
                    return Err(e.context(format!("write {}", file.display())));
                }
            }
            report.edited.push(file.clone());
        }
    }
    commit(&staged)?;
    for action in unique {
        if let Action::RemovePath(path) = action {
            let removed = remove_path(path)
                .with_context(|| format!("already removed: {:?}", report.removed))?;
            if removed {
                report.removed.push(path.clone());
            }
        }
    }
    Ok(report)
}

/// Removes a symlink itself (never its target), a file, or a whole directory tree. Returns
/// false (removes nothing) when an ancestor is a symlink, as deleting would reach into its target.
fn remove_path(path: &Path) -> anyhow::Result<bool> {
    // Rule: only the two nearest ancestors (managed root..target, e.g. cache/<mp>/<name>), so
    // system links (macOS /var) and a symlinked ~/.claude don't block removal.
    let linked = |a: &Path| fs::symlink_metadata(a).is_ok_and(|m| m.file_type().is_symlink());
    if path.ancestors().skip(1).take(2).any(linked) {
        return Ok(false);
    }
    let meta = match fs::symlink_metadata(path) {
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
        other => other.with_context(|| format!("stat {}", path.display()))?,
    };
    if meta.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
    .with_context(|| format!("remove {}", path.display()))?;
    Ok(true)
}

fn read_existing(file: &Path) -> anyhow::Result<Option<String>> {
    match fs::read_to_string(file) {
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        other => Ok(Some(
            other.with_context(|| format!("read {}", file.display()))?,
        )),
    }
}

/// An edit written to a temp file beside the real file, ready to rename over it.
struct Staged {
    target: PathBuf,
    tmp: PathBuf,
    original: String,
}

/// Temp file in the real file's directory, so a symlinked config (dotfiles) stays a symlink
/// and readers never see a half-written file. Keeps the original permissions.
fn stage(file: &Path, original: String, contents: &str) -> anyhow::Result<Staged> {
    let target = fs::canonicalize(file)?;
    let name = target
        .file_name()
        .context("no file name")?
        .to_string_lossy();
    let tmp = target.with_file_name(format!(".{name}.skillz-tmp"));
    let perms = fs::metadata(&target)?.permissions();
    let result = fs::write(&tmp, contents).and_then(|()| fs::set_permissions(&tmp, perms));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result?;
    Ok(Staged {
        target,
        tmp,
        original,
    })
}

/// Renames every temp file into place; if one fails, restores those already renamed (best
/// effort) and drops the rest.
fn commit(staged: &[Staged]) -> anyhow::Result<()> {
    for (i, s) in staged.iter().enumerate() {
        if let Err(e) = fs::rename(&s.tmp, &s.target) {
            for done in &staged[..i] {
                let _ = fs::write(&done.target, &done.original);
            }
            discard(&staged[i..]);
            return Err(e).with_context(|| format!("write {}", s.target.display()));
        }
    }
    Ok(())
}

fn discard(staged: &[Staged]) {
    for s in staged {
        let _ = fs::remove_file(&s.tmp);
    }
}

fn validate(file: &Path, contents: &str) -> anyhow::Result<()> {
    if file.extension().is_some_and(|e| e == "toml") {
        contents.parse::<toml_edit::DocumentMut>()?;
    } else {
        serde_json::from_str::<Value>(contents)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn json_after(edit: &Edit, doc: &Value) -> Value {
        let out = apply_edit(edit, &serde_json::to_string_pretty(doc).unwrap()).unwrap();
        serde_json::from_str(&out).unwrap()
    }

    fn toml_after(edit: &Edit, src: &str) -> toml_edit::DocumentMut {
        apply_edit(edit, src).unwrap().parse().unwrap()
    }

    fn hook(event: &str, command: &str) -> Edit {
        Edit::JsonRemoveHook {
            event: event.into(),
            command: command.into(),
        }
    }

    fn toml_hook(event: &str, command: &str) -> Edit {
        Edit::TomlRemoveHook {
            event: event.into(),
            command: command.into(),
        }
    }

    const HOOKS_TOML: &str = r#"[features]
x = 1

# start hooks
[[hooks.SessionStart]]
matcher = "startup"
[[hooks.SessionStart.hooks]]
command = "rtk"
[[hooks.SessionStart.hooks]]
command = "keep" # keep me

[[hooks.SessionStart]]
[[hooks.SessionStart.hooks]]
command = "rtk"

[hooks]
Stop = [{ hooks = [{ command = "rtk" }] }, { hooks = [] }, "odd"]

[hooks.state."a@b:x"]
trusted = true

[plugins."a@b"]
enabled = true
"#;

    #[test]
    fn toml_remove_hook_prunes_groups_and_events() {
        let out = apply_edit(&toml_hook("SessionStart", "rtk"), HOOKS_TOML).unwrap();
        assert!(out.contains("# start hooks\n") && out.contains("# keep me"));
        let doc: toml_edit::DocumentMut = out.parse().unwrap();
        let groups = doc["hooks"]["SessionStart"].as_array_of_tables().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups.get(0).unwrap()["matcher"].as_str(), Some("startup"));
        let handlers = groups.get(0).unwrap()["hooks"]
            .as_array_of_tables()
            .unwrap();
        let commands: Vec<_> = handlers.iter().map(|h| h["command"].as_str()).collect();
        assert_eq!(commands, [Some("keep")]);

        let doc = toml_after(&toml_hook("SessionStart", "keep"), &out);
        assert!(doc["hooks"].get("SessionStart").is_none());
        assert_eq!(doc["features"]["x"].as_integer(), Some(1));
        assert_eq!(doc["plugins"]["a@b"]["enabled"].as_bool(), Some(true));
        assert_eq!(
            doc["hooks"]["state"]["a@b:x"]["trusted"].as_bool(),
            Some(true)
        );
    }

    #[test]
    fn toml_remove_hook_inline_keeps_empty_and_odd_entries() {
        let doc = toml_after(&toml_hook("Stop", "rtk"), HOOKS_TOML);
        let stop = doc["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 2);
        assert_eq!(stop.get(1).and_then(|v| v.as_str()), Some("odd"));
    }

    #[test]
    fn toml_remove_hook_absent_is_verbatim() {
        for edit in [
            toml_hook("Nope", "rtk"),
            toml_hook("SessionStart", "nope"),
            toml_hook("state", "rtk"),
        ] {
            assert_eq!(apply_edit(&edit, HOOKS_TOML).unwrap(), HOOKS_TOML);
        }
        assert_eq!(
            apply_edit(&toml_hook("E", "c"), "a = 1\n").unwrap(),
            "a = 1\n"
        );
        assert!(apply_edit(&toml_hook("E", "c"), "[nope").is_err());
    }

    #[test]
    fn remove_hook_nested_prunes_groups_and_events() {
        let doc = json!({"other": 1, "hooks": {
            "PreToolUse": [
                {"matcher": "Bash", "hooks": [{"command": "rtk"}, {"command": "keep"}]},
                {"matcher": "Edit", "hooks": [{"command": "rtk"}]}
            ],
            "SessionStart": [{"hooks": [{"command": "rtk"}]}],
            "Stop": [{"hooks": []}]
        }});
        let after = json_after(&hook("PreToolUse", "rtk"), &doc);
        assert_eq!(
            after["hooks"]["PreToolUse"],
            json!([{"matcher": "Bash", "hooks": [{"command": "keep"}]}])
        );
        let after = json_after(&hook("SessionStart", "rtk"), &after);
        assert_eq!(
            after,
            json!({"other": 1, "hooks": {
                "PreToolUse": [{"matcher": "Bash", "hooks": [{"command": "keep"}]}],
                "Stop": [{"hooks": []}]
            }})
        );
    }

    #[test]
    fn remove_hook_flat_and_keeps_empty_hooks_object() {
        let doc = json!({"version": 1, "hooks": {"stop": [{"command": "bd"}, {"command": "bd"}]}});
        assert_eq!(
            json_after(&hook("stop", "bd"), &doc),
            json!({"version": 1, "hooks": {}})
        );
    }

    #[test]
    fn remove_hook_preserves_key_order() {
        let src =
            r#"{"z": 1, "hooks": {"b": [{"command": "x"}], "a": [{"command": "y"}]}, "m": 2}"#;
        let out: Value = serde_json::from_str(&apply_edit(&hook("b", "x"), src).unwrap()).unwrap();
        let keys: Vec<_> = out.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, ["z", "hooks", "m"]);
        assert_eq!(out["hooks"], json!({"a": [{"command": "y"}]}));
    }

    #[test]
    fn json_noops_return_input_verbatim() {
        let src = "{\"hooks\":{\"a\":[{\"command\":\"x\"}]},\"plugins\":{}}";
        let noops = [
            hook("missing", "x"),
            hook("a", "other"),
            Edit::JsonRemoveKey {
                path: strings(&["nope", "deeper"]),
            },
            Edit::JsonRemoveKey { path: vec![] },
            Edit::JsonRemovePluginInstall {
                id: "p@m".into(),
                project: None,
            },
        ];
        for edit in noops {
            assert_eq!(apply_edit(&edit, src).unwrap(), src, "{edit:?}");
        }
        assert_eq!(apply_edit(&hook("a", "x"), "{}").unwrap(), "{}");
        let not_object = r#"{"hooks": 3}"#;
        assert_eq!(apply_edit(&hook("a", "x"), not_object).unwrap(), not_object);
    }

    #[test]
    fn invalid_input_errors() {
        assert!(apply_edit(&hook("a", "x"), "{oops").is_err());
        assert!(
            apply_edit(
                &Edit::JsonRemoveKey {
                    path: strings(&["a"])
                },
                ""
            )
            .is_err()
        );
        assert!(
            apply_edit(
                &Edit::TomlRemoveKey {
                    path: strings(&["a"])
                },
                "a = "
            )
            .is_err()
        );
    }

    #[test]
    fn remove_json_key_nested_and_top_level() {
        let doc = json!({"enabledPlugins": {"x@y": true, "z@y": false}, "mp": {}});
        let edit = Edit::JsonRemoveKey {
            path: strings(&["enabledPlugins", "x@y"]),
        };
        assert_eq!(
            json_after(&edit, &doc),
            json!({"enabledPlugins": {"z@y": false}, "mp": {}})
        );
        let edit = Edit::JsonRemoveKey {
            path: strings(&["mp"]),
        };
        assert_eq!(
            json_after(&edit, &doc),
            json!({"enabledPlugins": {"x@y": true, "z@y": false}})
        );
    }

    #[test]
    fn output_is_pretty_with_trailing_newline() {
        let edit = Edit::JsonRemoveKey {
            path: strings(&["a"]),
        };
        let out = apply_edit(&edit, r#"{"a":1,"b":2}"#).unwrap();
        assert_eq!(
            out,
            serde_json::to_string_pretty(&json!({"b": 2})).unwrap() + "\n"
        );
    }

    #[test]
    fn remove_plugin_install_by_scope() {
        let doc = json!({"version": 2, "plugins": {
            "p@m": [
                {"scope": "user", "installPath": "/c"},
                {"scope": "project", "projectPath": "/proj"},
                {"scope": "local", "projectPath": "/other"}
            ],
            "q@m": [{"scope": "user"}]
        }});
        let user = Edit::JsonRemovePluginInstall {
            id: "p@m".into(),
            project: None,
        };
        assert_eq!(
            json_after(&user, &doc)["plugins"]["p@m"],
            json!([
                {"scope": "project", "projectPath": "/proj"},
                {"scope": "local", "projectPath": "/other"}
            ])
        );
        let proj = Edit::JsonRemovePluginInstall {
            id: "p@m".into(),
            project: Some("/proj".into()),
        };
        assert_eq!(
            json_after(&proj, &doc)["plugins"]["p@m"],
            json!([
                {"scope": "user", "installPath": "/c"},
                {"scope": "local", "projectPath": "/other"}
            ])
        );
        let last = Edit::JsonRemovePluginInstall {
            id: "q@m".into(),
            project: None,
        };
        let after = json_after(&last, &doc);
        assert!(after["plugins"].get("q@m").is_none());
        assert!(after["plugins"].get("p@m").is_some());
    }

    const CODEX: &str = r#"# top comment
model = "o3"

[marketplaces.mp]
source_type = "git" # inline comment
source = "https://github.com/org/repo.git"

# plugin comment
[plugins."x@mp"]
enabled = true

[plugins."keep@mp"]
enabled = false
"#;

    #[test]
    fn toml_remove_table_keeps_formatting() {
        let edit = Edit::TomlRemoveKey {
            path: strings(&["plugins", "x@mp"]),
        };
        let out = apply_edit(&edit, CODEX).unwrap();
        let doc: toml_edit::DocumentMut = out.parse().unwrap();
        assert!(doc["plugins"].get("x@mp").is_none());
        assert_eq!(doc["plugins"]["keep@mp"]["enabled"].as_bool(), Some(false));
        assert!(out.starts_with("# top comment\nmodel = \"o3\"\n"));
        assert!(out.contains("source_type = \"git\" # inline comment"));
    }

    #[test]
    fn toml_remove_dotted_and_inline_keys() {
        let edit = Edit::TomlRemoveKey {
            path: strings(&["plugins", "x@mp"]),
        };
        let dotted = "plugins.\"x@mp\".enabled = true\nplugins.\"k@mp\".enabled = true\n";
        let doc = toml_after(&edit, dotted);
        assert!(doc["plugins"].get("x@mp").is_none());
        assert!(doc["plugins"].get("k@mp").is_some());

        let inline = "plugins = { \"x@mp\" = { enabled = true }, other = 1 }\n";
        let doc = toml_after(&edit, inline);
        assert!(doc["plugins"].get("x@mp").is_none());
        assert_eq!(doc["plugins"]["other"].as_integer(), Some(1));

        let whole = Edit::TomlRemoveKey {
            path: strings(&["marketplaces", "mp"]),
        };
        let doc = toml_after(&whole, CODEX);
        assert!(doc.get("marketplaces").and_then(|m| m.get("mp")).is_none());
        assert!(doc["plugins"].get("x@mp").is_some());
    }

    #[test]
    fn toml_noops_return_input_verbatim() {
        for path in [
            strings(&["plugins", "absent"]),
            strings(&["model", "x"]),
            vec![],
        ] {
            assert_eq!(
                apply_edit(&Edit::TomlRemoveKey { path }, CODEX).unwrap(),
                CODEX
            );
        }
    }

    fn write(path: &Path, contents: &str) {
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn execute_removes_paths_and_skips_missing() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f.txt");
        let tree = dir.path().join("tree");
        write(&file, "x");
        fs::create_dir_all(tree.join("a/b")).unwrap();
        write(&tree.join("a/b/c"), "x");
        let missing = dir.path().join("missing");

        let report = execute(&[
            Action::RemovePath(file.clone()),
            Action::RemovePath(missing),
            Action::RemovePath(tree.clone()),
            Action::RemovePath(file.clone()),
        ])
        .unwrap();
        assert_eq!(report.removed, vec![file.clone(), tree.clone()]);
        assert!(report.edited.is_empty());
        assert!(!file.exists() && !tree.exists());
    }

    #[cfg(unix)]
    #[test]
    fn execute_removes_symlink_not_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("source-checkout");
        fs::create_dir(&target).unwrap();
        write(&target.join("SKILL.md"), "x");
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let dangling = dir.path().join("dangling");
        std::os::unix::fs::symlink(dir.path().join("gone"), &dangling).unwrap();

        let report = execute(&[
            Action::RemovePath(link.clone()),
            Action::RemovePath(dangling.clone()),
        ])
        .unwrap();
        assert_eq!(report.removed, vec![link.clone(), dangling.clone()]);
        assert!(fs::symlink_metadata(&link).is_err());
        assert!(fs::symlink_metadata(&dangling).is_err());
        assert!(target.join("SKILL.md").exists());
    }

    #[cfg(unix)]
    #[test]
    fn execute_skips_path_under_symlinked_ancestor() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("checkout");
        fs::create_dir_all(real.join("name")).unwrap();
        write(&real.join("name/SKILL.md"), "x");
        fs::create_dir(dir.path().join("cache")).unwrap();
        std::os::unix::fs::symlink(&real, dir.path().join("cache/mp")).unwrap();

        let report = execute(&[Action::RemovePath(dir.path().join("cache/mp/name"))]).unwrap();
        assert!(report.removed.is_empty());
        assert!(real.join("name/SKILL.md").exists());
    }

    #[test]
    fn commit_failure_restores_renamed_files() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a.json"), dir.path().join("b.json"));
        write(&a, "{}");
        write(&b, "[]");
        let staged = [
            stage(&a, "{}".into(), "{\"a\":1}").unwrap(),
            stage(&b, "[]".into(), "[1]").unwrap(),
        ];
        fs::remove_file(&staged[1].tmp).unwrap();
        assert!(commit(&staged).is_err());
        assert_eq!(fs::read_to_string(&a).unwrap(), "{}");
        assert_eq!(fs::read_to_string(&b).unwrap(), "[]");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn execute_edits_json_and_toml() {
        let dir = tempfile::tempdir().unwrap();
        let settings = dir.path().join("settings.json");
        let config = dir.path().join("config.toml");
        write(
            &settings,
            r#"{"enabledPlugins": {"x@mp": true}, "hooks": {"a": [{"command": "c"}]}}"#,
        );
        write(&config, CODEX);
        let key = Action::Edit {
            file: settings.clone(),
            edit: Edit::JsonRemoveKey {
                path: strings(&["enabledPlugins", "x@mp"]),
            },
        };
        let actions = [
            key.clone(),
            Action::Edit {
                file: settings.clone(),
                edit: hook("a", "c"),
            },
            key,
            Action::Edit {
                file: config.clone(),
                edit: Edit::TomlRemoveKey {
                    path: strings(&["plugins", "x@mp"]),
                },
            },
            Action::Edit {
                file: dir.path().join("absent.json"),
                edit: hook("a", "c"),
            },
        ];
        let report = execute(&actions).unwrap();
        assert_eq!(report.edited, vec![settings.clone(), config.clone()]);
        assert!(report.removed.is_empty());

        let json: Value = serde_json::from_str(&fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(json, json!({"enabledPlugins": {}, "hooks": {}}));
        let toml: toml_edit::DocumentMut = fs::read_to_string(&config).unwrap().parse().unwrap();
        assert!(toml["plugins"].get("x@mp").is_none());
    }

    #[test]
    fn execute_unchanged_edit_is_not_reported() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        let src = "{\"a\":1}";
        write(&file, src);
        let action = Action::Edit {
            file: file.clone(),
            edit: hook("x", "y"),
        };
        assert_eq!(execute(&[action]).unwrap(), Report::default());
        assert_eq!(fs::read_to_string(&file).unwrap(), src);
    }

    #[test]
    fn execute_fails_fast_on_invalid_file() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("bad.json");
        let later = dir.path().join("later");
        write(&bad, "{");
        write(&later, "x");
        let result = execute(&[
            Action::Edit {
                file: bad.clone(),
                edit: hook("a", "b"),
            },
            Action::RemovePath(later.clone()),
        ]);
        assert!(result.is_err());
        assert!(later.exists());
        assert_eq!(fs::read_to_string(&bad).unwrap(), "{");
    }

    #[test]
    fn execute_late_edit_failure_touches_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let doomed = dir.path().join("doomed");
        let good = dir.path().join("good.json");
        let bad = dir.path().join("bad.json");
        let src = r#"{"a": 1, "b": 2}"#;
        write(&doomed, "x");
        write(&good, src);
        write(&bad, "{");
        let result = execute(&[
            Action::RemovePath(doomed.clone()),
            Action::Edit {
                file: good.clone(),
                edit: Edit::JsonRemoveKey {
                    path: strings(&["a"]),
                },
            },
            Action::Edit {
                file: bad.clone(),
                edit: hook("a", "b"),
            },
        ]);
        assert!(result.is_err());
        assert!(doomed.exists());
        assert_eq!(fs::read_to_string(&good).unwrap(), src);
        assert_eq!(fs::read_to_string(&bad).unwrap(), "{");
    }

    #[cfg(unix)]
    #[test]
    fn execute_writes_through_symlink_and_keeps_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("dotfiles-settings.json");
        let link = dir.path().join("settings.json");
        write(&real, r#"{"a": 1, "b": 2}"#);
        fs::set_permissions(&real, fs::Permissions::from_mode(0o600)).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let report = execute(&[Action::Edit {
            file: link.clone(),
            edit: Edit::JsonRemoveKey {
                path: strings(&["a"]),
            },
        }])
        .unwrap();
        assert_eq!(report.edited, vec![link.clone()]);
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let json: Value = serde_json::from_str(&fs::read_to_string(&real).unwrap()).unwrap();
        assert_eq!(json, json!({"b": 2}));
        let mode = fs::metadata(&real).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn execute_rejects_format_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let json_named_toml = dir.path().join("x.toml");
        write(&json_named_toml, r#"{"a": {"b": 1}}"#);
        let action = Action::Edit {
            file: json_named_toml.clone(),
            edit: Edit::JsonRemoveKey {
                path: strings(&["a", "b"]),
            },
        };
        assert!(execute(&[action]).is_err());
        assert_eq!(
            fs::read_to_string(&json_named_toml).unwrap(),
            r#"{"a": {"b": 1}}"#
        );
    }

    #[test]
    fn execute_read_error_on_directory() {
        let dir = tempfile::tempdir().unwrap();
        let action = Action::Edit {
            file: dir.path().to_path_buf(),
            edit: hook("a", "b"),
        };
        assert!(execute(&[action]).is_err());
    }
}
