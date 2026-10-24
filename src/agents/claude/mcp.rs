//! Claude's MCP servers: `mcpServers` in `.claude.json` (user scope), the same key under
//! `projects.<path>` (local scope: private to you in that project) and the project's
//! `.mcp.json` (project scope: shared). `claude mcp add|remove --scope` manage all three.

use std::path::Path;

use serde_json::Value;

use super::files::read_json;
use crate::action::{Action, Edit};
use crate::env::Env;
use crate::mcp::Server;
use crate::model::{Agent, Installed, Installer, Kind, Scope};
use crate::profile::Spec;

pub(super) fn discover(env: &Env, scope: Scope) -> anyhow::Result<Vec<Installed>> {
    let state = env.claude_json();
    let json = read_json(&state)?;
    let project = env.project.display().to_string();
    let mut out = vec![];
    match scope {
        Scope::Global => {
            let user = Store {
                claude_scope: "user",
                file: &state,
                path: &["mcpServers"],
            };
            out.extend(server_installed(env, scope, &user, &json["mcpServers"]));
        }
        Scope::Project => {
            let local = Store {
                claude_scope: "local",
                file: &state,
                path: &["projects", project.as_str(), "mcpServers"],
            };
            let servers = &json["projects"][project.as_str()]["mcpServers"];
            out.extend(server_installed(env, scope, &local, servers));
            let shared = env.project.join(".mcp.json");
            let project_store = Store {
                claude_scope: "project",
                file: &shared,
                path: &["mcpServers"],
            };
            let servers = &read_json(&shared)?["mcpServers"];
            out.extend(server_installed(env, scope, &project_store, servers));
        }
    }
    Ok(out)
}

/// Where a set of servers is kept: removed with `claude mcp remove --scope <claude_scope>` and,
/// should that not run, by deleting the server's key under `path` in `file`.
struct Store<'a> {
    claude_scope: &'a str,
    file: &'a Path,
    path: &'a [&'a str],
}

fn server_installed(env: &Env, scope: Scope, store: &Store, servers: &Value) -> Vec<Installed> {
    let Store {
        claude_scope,
        file,
        path,
    } = *store;
    servers
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(name, v)| Server::from_json(name, v))
        .map(|server| {
            let mut key: Vec<String> = path.iter().map(|k| k.to_string()).collect();
            key.push(server.name.clone());
            let args = ["mcp", "remove", &server.name, "--scope", claude_scope];
            let mut removal: Vec<Action> =
                Action::native("claude", &args, &[&server.name], &env.project)
                    .into_iter()
                    .collect();
            removal.push(Action::Edit {
                file: file.to_path_buf(),
                edit: Edit::JsonRemoveKey { path: key },
            });
            Installed {
                details: crate::model::Details::Mcp {
                    config: file.to_path_buf(),
                    server: server.clone(),
                },
                hooks: Vec::new(),
                members: Vec::new(),
                id: format!("mcp:claude:{scope}:{claude_scope}:{}", server.name),
                name: server.name.clone(),
                kind: Kind::Mcp,
                agents: vec![Agent::Claude],
                scope,
                group: "mcp".into(),
                source: None,
                installer: Installer::ClaudeCli,
                origin: Default::default(),
                children: server.children(),
                removal,
            }
        })
        .collect()
}

/// `claude mcp add` for a profile entry. A project's servers go in the `local` scope, so they
/// never touch the `.mcp.json` the team shares.
pub(super) fn provision(entry: &Spec, scope: Scope, env: &Env) -> Option<Vec<Action>> {
    let Spec::Mcp {
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
    let claude_scope = match scope {
        Scope::Global => "user",
        Scope::Project => "local",
    };
    let mut cmd: Vec<String> = ["mcp", "add", "--scope", claude_scope]
        .map(String::from)
        .to_vec();
    match (command, url) {
        (Some(command), _) => {
            cmd.extend([name.clone(), "--".into(), command.clone()]);
            cmd.extend(args.iter().cloned());
        }
        (None, Some(url)) => {
            let transport = transport.clone().unwrap_or_else(|| "http".into());
            cmd.extend(["--transport".into(), transport, name.clone(), url.clone()]);
        }
        _ => return None,
    }
    Some(vec![Action::required("claude", cmd, &env.project)])
}
