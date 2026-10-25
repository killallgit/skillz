use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde_json::Value;

use super::json::{remove_array_items, remove_json_key};
use super::toml::toml_remove_key;
use super::{Action, Edit};
use crate::harness::hook_text::{self, Format};
use crate::harness::json_edit::edit_json;

/// Apply `edit` to `contents` of `file` (whose extension picks the format where the edit
/// depends on it). The result is guaranteed to parse in the same format.
pub fn apply_edit(file: &Path, edit: &Edit, contents: &str) -> anyhow::Result<String> {
    match edit {
        Edit::TomlRemoveKey { path } => toml_remove_key(contents, path),
        Edit::AddHook {
            event,
            matcher,
            command,
        } => {
            let hook = hook_text::HookEntry {
                event: event.clone(),
                matcher: matcher.clone(),
                command: command.clone(),
            };
            hook_text::add_hook(contents, Format::of(file), &hook)
        }
        Edit::RemoveHook { event, command } => {
            hook_text::remove_hook(contents, Format::of(file), event, command)
        }
        Edit::JsonRemoveKey { path } => edit_json(contents, |doc| remove_json_key(doc, path)),
        Edit::JsonRemoveArrayItems { path, fields } => {
            edit_json(contents, |doc| remove_array_items(doc, path, fields))
        }
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

/// The original text of a file (`None` when a hook edit will create it) and the planned one.
struct Texts {
    before: Option<String>,
    planned: String,
}

/// A hook can be added to a config that does not exist yet: it starts as an empty JSON object.
const NEW_FILE: &str = "{}\n";

/// Every edit to one real file, applied in memory. Symlinks are resolved, as `stage` writes
/// to the real file; `file` is the first path that named it.
pub(super) struct FilePlan {
    real: PathBuf,
    pub(super) file: PathBuf,
    texts: Option<Texts>,
}

impl FilePlan {
    fn open(file: &Path, edit: &Edit) -> anyhow::Result<Self> {
        let existing = crate::harness::read_optional::read_optional(file)?;
        let texts = match (existing, edit) {
            (Some(text), _) => Some(Texts {
                before: Some(text.clone()),
                planned: text,
            }),
            (None, Edit::AddHook { .. }) => Some(Texts {
                before: None,
                planned: NEW_FILE.to_string(),
            }),
            (None, _) => None,
        };
        Ok(Self {
            real: real_path(file),
            file: file.to_path_buf(),
            texts,
        })
    }

    /// Apply `edit` (a missing file is skipped), checking the result still parses.
    fn apply(&mut self, file: &Path, edit: &Edit) -> anyhow::Result<()> {
        let Some(texts) = &mut self.texts else {
            return Ok(());
        };
        let after = apply_edit(file, edit, &texts.planned)
            .with_context(|| format!("edit {}", file.display()))?;
        if after != texts.planned {
            validate(file, &after).with_context(|| format!("validate {}", file.display()))?;
            texts.planned = after;
        }
        Ok(())
    }

    /// `(before, planned)` when the plan changes the file.
    pub(super) fn change(&self) -> Option<(Option<&str>, &str)> {
        let texts = self.texts.as_ref()?;
        (texts.before.as_deref() != Some(texts.planned.as_str()))
            .then_some((texts.before.as_deref(), &texts.planned))
    }
}

fn real_path(file: &Path) -> PathBuf {
    fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf())
}

/// Plan every `Edit` in order, one `FilePlan` per real file. Any failure returns before the
/// filesystem is touched.
pub(super) fn plan_edits(
    actions: &[&Action],
    on_step: &mut dyn FnMut(&Action),
) -> anyhow::Result<Vec<FilePlan>> {
    let mut plans: Vec<FilePlan> = Vec::new();
    for action in actions {
        let Action::Edit { file, edit } = action else {
            continue;
        };
        on_step(action);
        plan_for(&mut plans, file, edit)?.apply(file, edit)?;
    }
    Ok(plans)
}

fn plan_for<'a>(
    plans: &'a mut Vec<FilePlan>,
    file: &Path,
    edit: &Edit,
) -> anyhow::Result<&'a mut FilePlan> {
    let real = real_path(file);
    let i = match plans.iter().position(|p| p.real == real) {
        Some(i) => i,
        None => {
            plans.push(FilePlan::open(file, edit)?);
            plans.len() - 1
        }
    };
    Ok(&mut plans[i])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::action::testutil::*;
    use serde_json::json;

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
}
