use std::ffi::OsStr;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::Context;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::hooks::{self, Format};

/// A single side effect needed to remove an augmentation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Delete a file, symlink, or directory tree. Missing paths are not an error.
    RemovePath(RemovalTarget),
    /// Rewrite a config file in place. Missing files are skipped.
    Edit { file: PathBuf, edit: Edit },
    /// `RemovePath`, but only when, after every edit is written, the JSON `file` (missing =
    /// empty) has nothing at `key` (e.g. `["plugins", "x@y"]`): a shared resource goes with
    /// its last reference, whichever selected items dropped it.
    RemovePathIfJsonAbsent {
        target: RemovalTarget,
        file: PathBuf,
        key: Vec<String>,
    },
}

/// A pure transformation of a config file's text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Edit {
    /// Remove every hook handler under `event` whose command equals `command`, in whichever
    /// layout the file uses (`hooks::remove_hook`). Prunes emptied groups and events.
    RemoveHook { event: String, command: String },
    /// Remove the object key at `path` (e.g. `["enabledPlugins", "x@y"]`). No-op if absent.
    JsonRemoveKey { path: Vec<String> },
    /// Remove every element of the array at `path` that `fields_match` `fields`. Removes the
    /// key at `path` when that empties the array. No-op if absent or not an array.
    JsonRemoveArrayItems {
        path: Vec<String>,
        fields: Vec<(String, Value)>,
    },
    /// Remove the TOML key/table at `path` (e.g. `["plugins", "x@y"]`), keeping formatting.
    TomlRemoveKey { path: Vec<String> },
}

/// A path strictly inside an owned root, built from plain segments. The only way to get a
/// `RemovePath`, so every deletion passes the same rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RemovalTarget {
    root: PathBuf,
    path: PathBuf,
}

impl RemovalTarget {
    /// `root` joined with `segments`, or `None` unless there is at least one segment and each
    /// is one plain UTF-8 name: non-empty, not `.`/`..`, no `/` or `\`. Config-supplied names
    /// go in as separate segments; a `&Path` iterates as its components.
    pub fn new<S: AsRef<OsStr>>(
        root: &Path,
        segments: impl IntoIterator<Item = S>,
    ) -> Option<Self> {
        let mut path = root.to_path_buf();
        for seg in segments {
            let seg = seg.as_ref().to_str()?;
            if seg.is_empty() || seg == "." || seg == ".." || seg.contains(['/', '\\']) {
                return None;
            }
            path.push(seg);
        }
        let target = Self {
            root: root.to_path_buf(),
            path,
        };
        target.inside().then_some(target)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn inside(&self) -> bool {
        self.path.strip_prefix(&self.root).is_ok_and(|rest| {
            rest.components().next().is_some()
                && rest.components().all(|c| matches!(c, Component::Normal(_)))
        })
    }
}

/// What `execute` actually changed.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    pub removed: Vec<PathBuf>,
    pub edited: Vec<PathBuf>,
}

/// Apply `edit` to `contents` of `file` (whose extension picks the format where the edit
/// depends on it). The result is guaranteed to parse in the same format.
pub fn apply_edit(file: &Path, edit: &Edit, contents: &str) -> anyhow::Result<String> {
    match edit {
        Edit::TomlRemoveKey { path } => toml_remove_key(contents, path),
        Edit::RemoveHook { event, command } => {
            hooks::remove_hook(contents, Format::of(file), event, command)
        }
        Edit::JsonRemoveKey { path } => edit_json(contents, |doc| remove_json_key(doc, path)),
        Edit::JsonRemoveArrayItems { path, fields } => {
            edit_json(contents, |doc| remove_array_items(doc, path, fields))
        }
    }
}

/// Parse, mutate, re-serialize. An edit that changes nothing returns `contents` verbatim so
/// callers can detect no-ops by comparing text.
pub(crate) fn edit_json(contents: &str, mutate: impl FnOnce(&mut Value)) -> anyhow::Result<String> {
    let original: Value = serde_json::from_str(contents).context("invalid JSON")?;
    let mut doc = original.clone();
    mutate(&mut doc);
    if doc == original {
        return Ok(contents.to_string());
    }
    Ok(serde_json::to_string_pretty(&doc)? + "\n")
}

/// The object holding the last key of `path`, and that key.
fn json_parent<'a>(
    doc: &'a mut Value,
    path: &'a [String],
) -> Option<(&'a mut Map<String, Value>, &'a str)> {
    let (last, parents) = path.split_last()?;
    let parent = parents
        .iter()
        .try_fold(doc, |v, key| v.get_mut(key.as_str()))?;
    Some((parent.as_object_mut()?, last))
}

fn remove_json_key(doc: &mut Value, path: &[String]) {
    if let Some((obj, last)) = json_parent(doc, path) {
        obj.shift_remove(last);
    }
}

fn remove_array_items(doc: &mut Value, path: &[String], fields: &[(String, Value)]) {
    let Some((obj, last)) = json_parent(doc, path) else {
        return;
    };
    let Some(items) = obj.get_mut(last).and_then(Value::as_array_mut) else {
        return;
    };
    let before = items.len();
    items.retain(|item| !fields_match(item, fields));
    if items.is_empty() && before > 0 {
        obj.shift_remove(last);
    }
}

/// True when every `fields` key of `item` holds exactly the paired value.
fn fields_match(item: &Value, fields: &[(String, Value)]) -> bool {
    fields.iter().all(|(k, v)| item.get(k) == Some(v))
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

/// `Some((original, planned))` text of one file, or `None` when the file is missing.
type Plan = Option<(String, String)>;

/// Dedupe, then plan every edit in memory (any failure returns before the filesystem is
/// touched), then write all edited files or none, then remove paths in their original order
/// (checking each `RemovePathIfJsonAbsent` condition against the written files).
pub fn execute(actions: &[Action]) -> anyhow::Result<Report> {
    let mut unique: Vec<&Action> = Vec::new();
    for action in actions {
        if !unique.contains(&action) {
            unique.push(action);
        }
    }

    // Per real file (symlinks resolved, as `stage` writes there), the first path naming it.
    let mut plans: Vec<(PathBuf, &PathBuf, Plan)> = Vec::new();
    for action in &unique {
        let Action::Edit { file, edit } = action else {
            continue;
        };
        let real = fs::canonicalize(file).unwrap_or_else(|_| file.clone());
        let i = match plans.iter().position(|(r, _, _)| *r == real) {
            Some(i) => i,
            None => {
                plans.push((real, file, read_existing(file)?.map(|s| (s.clone(), s))));
                plans.len() - 1
            }
        };
        let Some((_, current)) = &mut plans[i].2 else {
            continue;
        };
        let after =
            apply_edit(file, edit, current).with_context(|| format!("edit {}", file.display()))?;
        if after != *current {
            validate(file, &after).with_context(|| format!("validate {}", file.display()))?;
            *current = after;
        }
    }

    let mut report = Report::default();
    let mut staged = Vec::new();
    for (_, file, plan) in plans {
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
        let (target, wanted) = match action {
            Action::Edit { .. } => continue,
            Action::RemovePath(target) => (target, Ok(true)),
            Action::RemovePathIfJsonAbsent { target, file, key } => {
                (target, json_has(file, key).map(|has| !has))
            }
        };
        let removed = wanted
            .and_then(|wanted| {
                if wanted {
                    remove_path(target)
                } else {
                    Ok(false)
                }
            })
            .with_context(|| format!("already removed: {:?}", report.removed))?;
        if removed {
            report.removed.push(target.path.clone());
        }
    }
    Ok(report)
}

/// True when the JSON `file` holds a value at `key`; a missing file holds nothing.
fn json_has(file: &Path, key: &[String]) -> anyhow::Result<bool> {
    let Some(text) = read_existing(file)? else {
        return Ok(false);
    };
    let doc: Value =
        serde_json::from_str(&text).with_context(|| format!("parse {}", file.display()))?;
    Ok(key
        .iter()
        .try_fold(&doc, |v, k| v.get(k.as_str()))
        .is_some())
}

/// Removes a symlink itself (never its target), a file, or a whole directory tree. Returns
/// false (removes nothing) when the target is not strictly inside its root, or an ancestor is
/// a symlink, as deleting would reach into its target.
fn remove_path(target: &RemovalTarget) -> anyhow::Result<bool> {
    let path = &target.path;
    // Rule: only the two nearest ancestors (managed root..target, e.g. cache/<mp>/<name>), so
    // system links (macOS /var) and a symlinked ~/.claude don't block removal.
    let linked = |a: &Path| fs::symlink_metadata(a).is_ok_and(|m| m.file_type().is_symlink());
    if !target.inside() || path.ancestors().skip(1).take(2).any(linked) {
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
/// and readers never see a half-written file. The temp is created fresh with the original
/// mode (never wider, even briefly), then given exactly the original permissions.
fn stage(file: &Path, original: String, contents: &str) -> anyhow::Result<Staged> {
    let target = fs::canonicalize(file)?;
    let name = target
        .file_name()
        .context("no file name")?
        .to_string_lossy();
    let tmp = target.with_file_name(format!(".{name}.skillz-tmp"));
    let perms = fs::metadata(&target)?.permissions();
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        opts.mode(perms.mode());
    }
    // A leftover from a crashed run would block `create_new`.
    let _ = fs::remove_file(&tmp);
    let result = opts
        .open(&tmp)
        .and_then(|mut f| f.write_all(contents.as_bytes()))
        .and_then(|()| fs::set_permissions(&tmp, perms));
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
    if Format::of(file) == Format::Toml {
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

    const JSON: &str = "x.json";
    const TOML: &str = "x.toml";

    fn json_after(edit: &Edit, doc: &Value) -> Value {
        let src = serde_json::to_string_pretty(doc).unwrap();
        serde_json::from_str(&apply_edit(Path::new(JSON), edit, &src).unwrap()).unwrap()
    }

    fn toml_after(edit: &Edit, src: &str) -> toml_edit::DocumentMut {
        apply_edit(Path::new(TOML), edit, src)
            .unwrap()
            .parse()
            .unwrap()
    }

    fn hook(event: &str, command: &str) -> Edit {
        Edit::RemoveHook {
            event: event.into(),
            command: command.into(),
        }
    }

    #[test]
    fn remove_hook_format_follows_extension() {
        let json = r#"{"hooks": {"a": [{"command": "x"}]}}"#;
        let toml = "[[hooks.a]]\n[[hooks.a.hooks]]\ncommand = \"x\"\n";
        assert_eq!(
            json_after(&hook("a", "x"), &serde_json::from_str(json).unwrap()),
            json!({"hooks": {}})
        );
        let doc = toml_after(&hook("a", "x"), toml);
        assert!(doc.get("hooks").and_then(|h| h.get("a")).is_none());
        assert!(apply_edit(Path::new(TOML), &hook("a", "x"), json).is_err());
        assert!(apply_edit(Path::new(JSON), &hook("a", "x"), toml).is_err());
    }

    #[test]
    fn json_noops_return_input_verbatim() {
        let src = "{\"hooks\":{\"a\":[{\"command\":\"x\"}]},\"plugins\":{}}";
        let noops = [
            hook("missing", "x"),
            Edit::JsonRemoveKey {
                path: strings(&["nope", "deeper"]),
            },
            Edit::JsonRemoveKey { path: vec![] },
            Edit::JsonRemoveArrayItems {
                path: strings(&["plugins", "p@m"]),
                fields: vec![("scope".into(), json!("user"))],
            },
            Edit::JsonRemoveArrayItems {
                path: vec![],
                fields: vec![],
            },
        ];
        for edit in noops {
            assert_eq!(
                apply_edit(Path::new(JSON), &edit, src).unwrap(),
                src,
                "{edit:?}"
            );
        }
    }

    #[test]
    fn invalid_input_errors() {
        let key = Edit::JsonRemoveKey {
            path: strings(&["a"]),
        };
        assert!(apply_edit(Path::new(JSON), &key, "").is_err());
        let key = Edit::TomlRemoveKey {
            path: strings(&["a"]),
        };
        assert!(apply_edit(Path::new(TOML), &key, "a = ").is_err());
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
        let out = apply_edit(Path::new(JSON), &edit, r#"{"a":1,"b":2}"#).unwrap();
        assert_eq!(
            out,
            serde_json::to_string_pretty(&json!({"b": 2})).unwrap() + "\n"
        );
    }

    #[test]
    fn remove_array_items_matching_all_fields() {
        let doc = json!({"a": {
            "xs": [{"k": "1", "n": 2}, {"k": "1", "n": 3}, {"k": "2", "n": 2}, "scalar"],
            "one": [{"k": "1"}]
        }});
        let remove = |path: &[&str], fields: &[(&str, Value)]| Edit::JsonRemoveArrayItems {
            path: strings(path),
            fields: fields
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
        };
        let both = remove(&["a", "xs"], &[("k", json!("1")), ("n", json!(2))]);
        assert_eq!(
            json_after(&both, &doc)["a"]["xs"],
            json!([{"k": "1", "n": 3}, {"k": "2", "n": 2}, "scalar"])
        );
        let k1 = remove(&["a", "xs"], &[("k", json!("1"))]);
        assert_eq!(
            json_after(&k1, &doc)["a"]["xs"],
            json!([{"k": "2", "n": 2}, "scalar"])
        );
        let emptied = json_after(&remove(&["a", "one"], &[("k", json!("1"))]), &doc);
        assert!(emptied["a"].get("one").is_none());
        assert!(emptied["a"].get("xs").is_some());
        let src = r#"{"a":{"xs":[],"s":"x"}}"#;
        for edit in [
            remove(&["a", "xs"], &[]),
            remove(&["a", "s"], &[]),
            remove(&["a", "xs", "deeper"], &[]),
            remove(&["missing", "xs"], &[]),
        ] {
            assert_eq!(
                apply_edit(Path::new(JSON), &edit, src).unwrap(),
                src,
                "{edit:?}"
            );
        }
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
        let out = apply_edit(Path::new(TOML), &edit, CODEX).unwrap();
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
                apply_edit(Path::new(TOML), &Edit::TomlRemoveKey { path }, CODEX).unwrap(),
                CODEX
            );
        }
    }

    fn write(path: &Path, contents: &str) {
        fs::write(path, contents).unwrap();
    }

    fn rm(path: &Path) -> Action {
        let target = RemovalTarget::new(path.parent().unwrap(), [path.file_name().unwrap()]);
        Action::RemovePath(target.unwrap())
    }

    #[test]
    fn removal_target_accepts_only_plain_segments() {
        let root = Path::new("/r");
        for bad in ["", ".", "..", "a/b", "a\\b", "/abs"] {
            assert!(RemovalTarget::new(root, [bad]).is_none(), "{bad}");
            assert!(RemovalTarget::new(root, ["ok", bad]).is_none(), "{bad}");
        }
        assert!(RemovalTarget::new(root, Vec::<&str>::new()).is_none());
        assert!(RemovalTarget::new(root, Path::new("a/../b")).is_none());
        for good in ["a..b", ".hidden"] {
            let target = RemovalTarget::new(root, [good]).unwrap();
            assert_eq!(target.path(), root.join(good));
        }
        let nested = RemovalTarget::new(root, Path::new("a/b")).unwrap();
        assert_eq!(nested.path(), Path::new("/r/a/b"));
    }

    #[test]
    fn execute_skips_target_outside_its_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        let victim = dir.path().join("victim");
        fs::create_dir(&root).unwrap();
        write(&victim, "x");
        let escaped = [
            RemovalTarget {
                root: root.clone(),
                path: victim.clone(),
            },
            RemovalTarget {
                root: root.clone(),
                path: root.join("../victim"),
            },
            RemovalTarget {
                root: root.clone(),
                path: root.clone(),
            },
        ];
        let report = execute(&escaped.map(Action::RemovePath)).unwrap();
        assert!(report.removed.is_empty());
        assert!(victim.exists() && root.exists());
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

        let report = execute(&[rm(&file), rm(&missing), rm(&tree), rm(&file)]).unwrap();
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

        let report = execute(&[rm(&link), rm(&dangling)]).unwrap();
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

        let report = execute(&[rm(&dir.path().join("cache/mp/name"))]).unwrap();
        assert!(report.removed.is_empty());
        assert!(real.join("name/SKILL.md").exists());
    }

    #[test]
    fn execute_removes_conditional_path_only_once_key_is_gone() {
        let dir = tempfile::tempdir().unwrap();
        let index = dir.path().join("index.json");
        write(&index, r#"{"plugins": {"a@m": [1], "b@m": [2]}}"#);
        let dirs = ["a", "b", "c"].map(|d| {
            fs::create_dir(dir.path().join(d)).unwrap();
            dir.path().join(d)
        });
        let unless = |path: &Path, file: &Path, id: &str| Action::RemovePathIfJsonAbsent {
            target: RemovalTarget::new(path.parent().unwrap(), [path.file_name().unwrap()])
                .unwrap(),
            file: file.to_path_buf(),
            key: strings(&["plugins", id]),
        };
        let report = execute(&[
            unless(&dirs[0], &index, "a@m"),
            unless(&dirs[1], &index, "b@m"),
            unless(&dirs[2], &dir.path().join("missing.json"), "c@m"),
            Action::Edit {
                file: index.clone(),
                edit: Edit::JsonRemoveKey {
                    path: strings(&["plugins", "a@m"]),
                },
            },
        ])
        .unwrap();
        assert_eq!(report.removed, vec![dirs[0].clone(), dirs[2].clone()]);
        assert!(!dirs[0].exists() && dirs[1].exists() && !dirs[2].exists());

        write(&index, "{");
        assert!(execute(&[unless(&dirs[1], &index, "b@m")]).is_err());
        assert!(dirs[1].exists());
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
            rm(&later),
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
            rm(&doomed),
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

    #[cfg(unix)]
    #[test]
    fn stage_creates_fresh_temp_with_original_mode() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        let victim = dir.path().join("victim");
        write(&file, "a = 1\n");
        write(&victim, "keep");
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
        // A stale temp left as a link must be replaced, not written through.
        std::os::unix::fs::symlink(&victim, dir.path().join(".config.toml.skillz-tmp")).unwrap();

        let staged = stage(&file, "a = 1\n".into(), "b = 2\n").unwrap();
        let meta = fs::symlink_metadata(&staged.tmp).unwrap();
        assert!(meta.is_file());
        assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        assert_eq!(fs::read_to_string(&staged.tmp).unwrap(), "b = 2\n");
        assert_eq!(fs::read_to_string(&victim).unwrap(), "keep");
    }

    #[cfg(unix)]
    #[test]
    fn execute_merges_edits_to_one_file_through_a_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("a.json");
        let alias = dir.path().join("b.json");
        write(&real, r#"{"x": 1, "y": 2, "z": 3}"#);
        std::os::unix::fs::symlink(&real, &alias).unwrap();
        let remove = |file: &Path, key: &str| Action::Edit {
            file: file.to_path_buf(),
            edit: Edit::JsonRemoveKey {
                path: strings(&[key]),
            },
        };

        execute(&[remove(&real, "x"), remove(&alias, "y")]).unwrap();
        let json: Value = serde_json::from_str(&fs::read_to_string(&real).unwrap()).unwrap();
        assert_eq!(json, json!({"z": 3}));
        assert!(fs::symlink_metadata(&alias).unwrap().is_symlink());
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
