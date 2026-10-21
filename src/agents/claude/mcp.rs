//! Claude's MCP servers: `mcpServers` in `.claude.json` (user scope), the same key under
//! `projects.<path>` (local scope: private to you in that project) and the project's
//! `.mcp.json` (project scope: shared). `claude mcp add|remove --scope` manage all three.

use std::path::Path;

use serde_json::Value;

use super::files::read_json;
use crate::action::{Action, Edit};
use crate::env::Env;
use crate::mcp::Server;
use crate::model::{Agent, Augmentation, Installer, Kind, Scope};
use crate::profile::Entry;

pub(super) fn discover(env: &Env, scope: Scope) -> anyhow::Result<Vec<Augmentation>> {
    let state = env.claude_json();
    let json = read_json(&state)?;
    let project = env.project.display().to_string();
    let mut out = vec![];
    match scope {
        Scope::Global => out.extend(server_augmentation(
            env,
            scope,
            "user",
            &json["mcpServers"],
            &state,
            &["mcpServers"],
        )),
        Scope::Project => {
            let local = &json["projects"][project.as_str()]["mcpServers"];
            let path = ["projects", project.as_str(), "mcpServers"];
            out.extend(server_augmentation(
                env, scope, "local", local, &state, &path,
            ));
            let shared = env.project.join(".mcp.json");
            out.extend(server_augmentation(
                env,
                scope,
                "project",
                &read_json(&shared)?["mcpServers"],
                &shared,
                &["mcpServers"],
            ));
        }
    }
    Ok(out)
}

/// The servers under `servers`, each removed with `claude mcp remove --scope <claude_scope>`
/// and, should that not run, by deleting its key at `path` in `file`.
fn server_augmentation(
    env: &Env,
    scope: Scope,
    claude_scope: &str,
    servers: &Value,
    file: &Path,
    path: &[&str],
) -> Vec<Augmentation> {
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
            Augmentation {
                id: format!("mcp:claude:{scope}:{claude_scope}:{}", server.name),
                name: server.name.clone(),
                kind: Kind::Mcp,
                agents: vec![Agent::Claude],
                scope,
                group: "mcp".into(),
                source: None,
                installer: Installer::ClaudeCli,
                children: server.children(),
                removal,
            }
        })
        .collect()
}

/// `claude mcp add` for a profile entry. A project's servers go in the `local` scope, so they
/// never touch the `.mcp.json` the team shares.
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
