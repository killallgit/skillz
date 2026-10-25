use std::path::Path;

use crate::harness::action::{Action, Edit};
use crate::model::Scope;

/// `codex plugin …` always works on the user config, so only global items use it; a project's
/// `.codex/config.toml` is edited directly.
pub(super) fn native(scope: Scope, config: &Path, args: &[&str], name: &str) -> Vec<Action> {
    let home = config.parent();
    match (scope, home) {
        (Scope::Global, Some(cwd)) => Action::native("codex", args, &[name], cwd)
            .into_iter()
            .collect(),
        _ => vec![],
    }
}

pub(super) fn toml_remove(config: &Path, path: &[&str]) -> Action {
    Action::Edit {
        file: config.to_path_buf(),
        edit: Edit::TomlRemoveKey {
            path: path.iter().map(|k| k.to_string()).collect(),
        },
    }
}
