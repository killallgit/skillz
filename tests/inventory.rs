//! What discovery finds in a fake home + project.

mod common;

use std::path::PathBuf;

use common::*;
use skillz::action::{Action, execute};
use skillz::agents::{self, Inventory};
use skillz::env::Env;
use skillz::model::{Agent, Installed, Kind, Scope};
use skillz::runner::NoCommands;

fn inventory(env: &Env, scopes: &[Scope]) -> Inventory {
    agents::inventory(&agents::registry(), env, scopes)
}

fn skills(env: &Env, scopes: &[Scope]) -> Vec<Installed> {
    inventory(env, scopes)
        .items
        .into_iter()
        .filter(|i| i.kind == Kind::Skill)
        .collect()
}

fn hooks(env: &Env) -> Vec<(String, Agent, Scope)> {
    inventory(env, &[Scope::Global, Scope::Project])
        .items
        .into_iter()
        .filter(|i| i.kind == Kind::Hook)
        .map(|i| (i.name.clone(), i.agents[0], i.scope))
        .collect()
}

fn removed_paths(item: &Installed) -> Vec<PathBuf> {
    item.removal
        .iter()
        .filter_map(|act| match act {
            Action::RemovePath(target) => Some(target.path().to_path_buf()),
            _ => None,
        })
        .collect()
}

/// The world plus a project-level link to the same global skill store.
fn world_with_project_link() -> (World, Env, PathBuf) {
    let w = world();
    let env = w.env();
    let link = env.project.join(".claude/skills/tdd");
    std::fs::create_dir_all(link.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(env.home.join(".agents/skills/tdd"), &link).unwrap();
    (w, env, link)
}

#[test]
fn discovery_raises_no_warnings() {
    let w = world();

    let inv = inventory(&w.env(), &[Scope::Global, Scope::Project]);

    assert!(inv.warnings.is_empty(), "{:?}", inv.warnings);
}

#[test]
fn a_skill_linked_into_two_agents_is_one_item() {
    let w = world();

    let found = skills(&w.env(), &[Scope::Global, Scope::Project]);

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "tdd");
}

#[test]
fn a_linked_skill_names_every_agent_it_is_linked_into() {
    let w = world();

    let found = skills(&w.env(), &[Scope::Global, Scope::Project]);

    assert!(found[0].agents.contains(&Agent::Claude));
    assert!(found[0].agents.contains(&Agent::Codex));
}

#[test]
fn each_hook_tool_is_found_once_per_agent_and_scope() {
    let w = world();

    let mut found = hooks(&w.env());

    found.sort();
    let mut expected = vec![
        ("rtk".to_string(), Agent::Claude, Scope::Global),
        ("bd".to_string(), Agent::Claude, Scope::Global),
        ("bd".to_string(), Agent::Codex, Scope::Global),
        ("bd".to_string(), Agent::Codex, Scope::Project),
    ];
    expected.sort();
    assert_eq!(found, expected);
}

#[test]
fn a_project_link_into_the_global_store_is_its_own_project_item() {
    let (_w, env, link) = world_with_project_link();

    let found = skills(&env, &[Scope::Project]);

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].scope, Scope::Project);
    assert_eq!(removed_paths(&found[0]), vec![link]);
}

#[test]
fn with_both_scopes_the_global_skill_and_its_project_link_are_two_items() {
    let (_w, env, _link) = world_with_project_link();

    let found = skills(&env, &[Scope::Global, Scope::Project]);

    let mut scopes: Vec<Scope> = found.iter().map(|a| a.scope).collect();
    scopes.sort_by_key(|s| *s == Scope::Project);
    assert_eq!(scopes, [Scope::Global, Scope::Project]);
}

#[test]
fn removing_both_scopes_deletes_the_link_and_the_store() {
    let (_w, env, link) = world_with_project_link();
    let plan: Vec<Action> = skills(&env, &[Scope::Global, Scope::Project])
        .iter()
        .flat_map(|a| a.removal.clone())
        .collect();

    execute(&plan, &NoCommands).unwrap();

    assert!(!present(&link));
    assert!(!env.home.join(".agents/skills/tdd").exists());
}
