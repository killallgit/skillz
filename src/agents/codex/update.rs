use std::path::Path;

use crate::harness::action::Action;
use crate::model::{Installed, Installer, Kind, Scope, split_plugin_id};

pub(super) fn steps(item: &Installed, cwd: &Path) -> Vec<Action> {
    if item.installer != Installer::CodexCli || item.scope != Scope::Global {
        return vec![];
    }
    let run = |args: &[&str]| {
        Action::required("codex", args.iter().map(|s| s.to_string()).collect(), cwd)
    };
    match item.kind {
        Kind::Marketplace => vec![run(&["plugin", "marketplace", "upgrade", &item.name])],
        Kind::Plugin => {
            let marketplace = split_plugin_id(&item.name).1;
            if marketplace.is_empty() || marketplace.starts_with('-') {
                return vec![];
            }
            vec![
                run(&["plugin", "marketplace", "upgrade", marketplace]),
                run(&["plugin", "add", &item.name]),
            ]
        }
        _ => vec![],
    }
}
