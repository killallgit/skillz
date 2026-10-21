//! Side effects that remove an augmentation, and the executor that applies them.

mod edit;
mod execute;
mod files;
mod json;
mod removal_target;
mod stage;
#[cfg(test)]
mod testutil;
mod toml;

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

pub use edit::apply_edit;
pub use execute::{execute, execute_with};
pub(crate) use json::edit_json;
pub use removal_target::RemovalTarget;

/// A single side effect of changing what is installed: removing an augmentation, or (from a
/// profile) installing one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Run the item's own uninstall (`claude plugin uninstall …`). Runs before any edit or
    /// removal; a command that exits non-zero aborts the whole plan with nothing else done.
    /// With `fallback`, a program that cannot run at all (not installed) is skipped instead,
    /// and the file steps of the plan do the removal.
    Run {
        program: String,
        args: Vec<String>,
        cwd: PathBuf,
        fallback: bool,
    },
    /// Delete a file, symlink, or directory tree. Missing paths are not an error.
    RemovePath(RemovalTarget),
    /// Rewrite a config file in place. Missing files are skipped.
    Edit { file: PathBuf, edit: Edit },
    /// Install subagent definitions: fetch `source` (a directory relative to `cwd`, else a git
    /// repo), copy the `ext` files called `names` into `to`, note the source beside them.
    /// Runs with the commands, before any edit.
    CopyAgents {
        source: String,
        cwd: PathBuf,
        names: Vec<String>,
        ext: String,
        to: PathBuf,
    },
    /// `RemovePath`, but only when, after every edit is written, the JSON `file` (missing =
    /// empty) has nothing at `key` (e.g. `["plugins", "x@y"]`): a shared resource goes with
    /// its last reference, whichever selected items dropped it.
    RemovePathIfJsonAbsent {
        target: RemovalTarget,
        file: PathBuf,
        key: Vec<String>,
    },
}

impl Action {
    /// A `Run` with `fallback`. `names` are the args taken from config files; `None` when one
    /// is empty or option-like (`-x`), so a crafted id never becomes a flag of the tool.
    pub fn native(program: &str, args: &[&str], names: &[&str], cwd: &Path) -> Option<Action> {
        if names.iter().any(|n| n.is_empty() || n.starts_with('-')) {
            return None;
        }
        Some(Action::Run {
            program: program.into(),
            args: args.iter().map(|a| a.to_string()).collect(),
            cwd: cwd.into(),
            fallback: true,
        })
    }

    /// A `Run` that must work: a program that cannot start is an error, not a skip. Installs use
    /// this; there is no file step to fall back on. Callers vet the arguments first (see
    /// `Entry::validate`).
    pub fn required(program: &str, args: Vec<String>, cwd: &Path) -> Action {
        Action::Run {
            program: program.into(),
            args,
            cwd: cwd.into(),
            fallback: false,
        }
    }

    /// One line saying what this step does, for plans and the info modal.
    pub fn describe(&self) -> String {
        match self {
            Action::Run { program, args, .. } => format!("run {program} {}", args.join(" ")),
            Action::CopyAgents {
                source, names, to, ..
            } => format!(
                "copy {} from {source} into {}",
                names.join(","),
                to.display()
            ),
            Action::RemovePath(t) => format!("rm {}", t.path().display()),
            Action::RemovePathIfJsonAbsent { target, file, key } => format!(
                "rm {} (if {} then has no {})",
                target.path().display(),
                file.display(),
                key.join(".")
            ),
            Action::Edit { file, edit } => {
                format!("edit {} ({})", file.display(), edit.describe())
            }
        }
    }
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
    /// Add a hook handler under `event` (in the group with the same `matcher`), creating the
    /// file and whatever is missing in it. A handler that is already there stays as it is.
    AddHook {
        event: String,
        matcher: Option<String>,
        command: String,
    },
    /// Remove the TOML key/table at `path` (e.g. `["plugins", "x@y"]`), keeping formatting.
    TomlRemoveKey { path: Vec<String> },
}

impl Edit {
    fn describe(&self) -> String {
        match self {
            Edit::RemoveHook { event, command } => format!("remove hook {event}: {command}"),
            Edit::AddHook { event, command, .. } => format!("add hook {event}: {command}"),
            Edit::JsonRemoveKey { path } | Edit::TomlRemoveKey { path } => {
                format!("remove {}", path.join("."))
            }
            Edit::JsonRemoveArrayItems { path, fields } => {
                let when: Vec<String> = fields.iter().map(|(k, v)| format!("{k}={v}")).collect();
                format!("remove {}[{}]", path.join("."), when.join(","))
            }
        }
    }
}

/// What `execute` actually changed.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    /// Each command run, as `program arg…`.
    pub ran: Vec<String>,
    /// Commands skipped because their program could not run (`fallback`).
    pub skipped: Vec<String>,
    pub removed: Vec<PathBuf>,
    pub edited: Vec<PathBuf>,
    /// Subagent files written.
    pub copied: Vec<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_refuses_empty_and_option_like_names() {
        let at = |name: &str| Action::native("tool", &["rm", name], &[name], Path::new("/"));
        assert!(at("ok").is_some());
        assert!(at("").is_none());
        assert!(at("-x").is_none());
    }
}
