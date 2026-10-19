use std::path::PathBuf;

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
    JsonRemovePluginInstall { id: String, project: Option<PathBuf> },
    /// Remove the TOML key/table at `path` (e.g. `["plugins", "x@y"]`), keeping formatting.
    TomlRemoveKey { path: Vec<String> },
}

/// What `execute` actually changed.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    pub removed: Vec<PathBuf>,
    pub edited: Vec<PathBuf>,
}

/// Apply `edit` to `contents`. The result is guaranteed to parse in the same format.
pub fn apply_edit(edit: &Edit, contents: &str) -> anyhow::Result<String> {
    let _ = (edit, contents);
    todo!("lane: config-core")
}

/// Run actions in order. Duplicate actions run once; edits are validated before writing.
pub fn execute(actions: &[Action]) -> anyhow::Result<Report> {
    let _ = actions;
    todo!("lane: config-core")
}
