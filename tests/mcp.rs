//! MCP servers across Claude's `.claude.json` and `.mcp.json` and Codex's `config.toml`.

mod common;

use common::*;
use serde_json::Value;
use toml_edit::DocumentMut;

/// Every string anywhere in `value`.
fn strings_in(value: &Value) -> Vec<&str> {
    match value {
        Value::String(s) => vec![s.as_str()],
        Value::Array(items) => items.iter().flat_map(strings_in).collect(),
        Value::Object(map) => map
            .iter()
            .flat_map(|(k, v)| std::iter::once(k.as_str()).chain(strings_in(v)))
            .collect(),
        _ => vec![],
    }
}

fn listed_mcp_servers() -> Value {
    let w = mcp_world();
    let (r, p) = (Recorder::default(), Scripted::yes());
    let cmd = skillz::cli::Command::List(skillz::cli::ListArgs {
        target: None,
        project: None,
        all: true,
        json: true,
        check: false,
        available: false,
        filter: only_mcp(),
        interactive: false,
    });
    serde_json::from_slice(&run_captured(cmd, &deps(&w, &r, &p))).unwrap()
}

/// Removes the one server `name` with the native commands recorded; returns what is left.
fn after_removing(name: &str) -> (World, Vec<Call>) {
    let w = mcp_world();
    let (r, p) = (Recorder::default(), Scripted::yes());
    run_quiet(uninstall(name, only_mcp(), true), &deps(&w, &r, &p)).unwrap();
    (w, r.calls())
}

fn mcp_remove_scopes(calls: &[Call]) -> Vec<&str> {
    calls
        .iter()
        .filter(|(prog, args)| prog == "claude" && args[..2] == ["mcp", "remove"])
        .map(|(_, args)| args.last().unwrap().as_str())
        .collect()
}

fn local_servers(w: &World) -> Value {
    let project = w.project.canonicalize().unwrap().display().to_string();
    read_json(&w.home.join(".claude.json"))["projects"][project.as_str()]["mcpServers"].clone()
}

#[test]
fn servers_are_listed_from_every_agent_file() {
    let items = listed_mcp_servers();

    let mut names = names_of(items.as_array().unwrap());

    names.sort();
    assert_eq!(names, ["cx", "keyed", "local-web", "shared", "user-fs"]);
}

#[test]
fn listed_servers_never_carry_secret_values() {
    let items = listed_mcp_servers();

    assert!(!strings_in(&items).iter().any(|s| s.contains("hunter2")));
}

#[test]
fn removing_a_user_server_uses_claudes_user_scope() {
    let (_w, calls) = after_removing("user-fs");

    assert_eq!(mcp_remove_scopes(&calls), ["user"]);
}

#[test]
fn removing_a_local_server_uses_claudes_local_scope() {
    let (_w, calls) = after_removing("local-web");

    assert_eq!(mcp_remove_scopes(&calls), ["local"]);
}

#[test]
fn removing_a_shared_server_uses_claudes_project_scope() {
    let (_w, calls) = after_removing("shared");

    assert_eq!(mcp_remove_scopes(&calls), ["project"]);
}

#[test]
fn removing_a_codex_server_uses_codexs_own_command() {
    let (_w, calls) = after_removing("cx");

    assert!(
        calls
            .iter()
            .any(|(prog, args)| prog == "codex" && args[..2] == ["mcp", "remove"])
    );
}

#[test]
fn removing_a_user_server_drops_it_from_claude_json() {
    let (w, _) = after_removing("user-fs");

    let state = read_json(&w.home.join(".claude.json"));

    assert!(state["mcpServers"].get("user-fs").is_none());
}

#[test]
fn removing_a_local_server_drops_only_that_server() {
    let (w, _) = after_removing("local-web");

    let local = local_servers(&w);

    assert!(local.get("local-web").is_none());
    assert!(local.get("keyed").is_some());
}

#[test]
fn removing_a_shared_server_drops_it_from_mcp_json() {
    let (w, _) = after_removing("shared");

    let shared = read_json(&w.project.join(".mcp.json"));

    assert!(shared["mcpServers"].get("shared").is_none());
}

#[test]
fn removing_a_codex_server_drops_it_from_the_codex_config() {
    let (w, _) = after_removing("cx");

    let config: DocumentMut = std::fs::read_to_string(w.home.join(".codex/config.toml"))
        .unwrap()
        .parse()
        .unwrap();

    assert!(
        config
            .get("mcp_servers")
            .and_then(|s| s.get("cx"))
            .is_none()
    );
}
