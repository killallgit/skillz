//! Codex's MCP servers: the `[mcp_servers.<name>]` tables of `config.toml`, managed with
//! `codex mcp add|remove`.

use std::path::Path;

use toml_edit::DocumentMut;

use super::config::table_keys;
use super::removal::{native, toml_remove};
use crate::action::Action;
use crate::env::Env;
use crate::mcp::Server;
use crate::model::{Agent, Augmentation, Installer, Kind, Scope};
use crate::profile::Entry;

/// The servers in the config, each removed by `codex mcp remove` (user config only) and by
/// deleting its table.
pub(super) fn discover(doc: &DocumentMut, scope: Scope, config: &Path) -> Vec<Augmentation> {
    table_keys(doc, "mcp_servers")
        .into_iter()
        .filter_map(|(name, item)| Server::from_toml(name, item.as_table_like()?))
        .map(|server| server_augmentation(&server, scope, config))
        .collect()
}

fn server_augmentation(server: &Server, scope: Scope, config: &Path) -> Augmentation {
    let mut removal = native(
        scope,
        config,
        &["mcp", "remove", &server.name],
        &server.name,
    );
    removal.push(toml_remove(config, &["mcp_servers", &server.name]));
    Augmentation {
        hooks: Vec::new(),
        id: format!("mcp:codex:{scope}:{}", server.name),
        name: server.name.clone(),
        kind: Kind::Mcp,
        agents: vec![Agent::Codex],
        scope,
        group: "mcp".into(),
        source: None,
        installer: Installer::CodexCli,
        children: server.children(),
        removal,
    }
}

/// `codex mcp add` for a profile entry. Like the plugin commands it only edits the user config,
/// so a project has no way in; Codex has no SSE transport.
pub(super) fn provision(entry: &Entry, scope: Scope, env: &Env) -> Option<Vec<Action>> {
    let Entry::Mcp {
        name,
        command,
        args,
        url,
        transport,
        ..
    } = entry
    else {
        return None;
    };
    if scope != Scope::Global {
        return None;
    }
    let mut cmd: Vec<String> = vec!["mcp".into(), "add".into(), name.clone()];
    match (command, url) {
        (Some(command), _) => {
            cmd.extend(["--".into(), command.clone()]);
            cmd.extend(args.iter().cloned());
        }
        (None, Some(url)) if transport.as_deref().unwrap_or("http") == "http" => {
            cmd.extend(["--url".into(), url.clone()]);
        }
        _ => return None,
    }
    Some(vec![Action::required("codex", cmd, &env.codex_cwd())])
}
