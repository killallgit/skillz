//! A project folder that moves: hand-made skills recorded by their absolute directory are
//! followed by `skillz sync` instead of being duplicated, and an entry whose folder is gone
//! fails clearly instead of reaching npx.

mod common;

use std::fs;

use common::*;
use serde_json::Value;
use skillz::harness::Spec;

const SKILL: &str = "---\nname: mine\ndescription: d\n---\n";

fn old_skills(sb: &Sandbox) -> String {
    sb.project.join(".agents/skills").to_string_lossy().into()
}

fn profile_text(source: &str) -> String {
    format!("[[entry]]\nkind = \"skill\"\nsource = \"{source}\"\nnames = [\"mine\"]\n")
}

/// A project with a hand-made skill, synced, and a profile that names its directory.
fn synced_sandbox() -> Sandbox {
    let sb = Sandbox::new();
    write(&sb.project.join(".agents/skills/mine/SKILL.md"), SKILL);
    sb.write_profile("mine", &profile_text(&old_skills(&sb)));
    assert!(sb.skillz(&["sync"]).status.success());
    sb
}

/// The same project, found one directory up from where it was.
fn moved_sandbox() -> Sandbox {
    let mut sb = synced_sandbox();
    let new_project = sb.project.parent().unwrap().join("moved");
    fs::rename(&sb.project, &new_project).unwrap();
    sb.project = new_project;
    sb
}

fn new_skills(sb: &Sandbox) -> String {
    sb.project.join(".agents/skills").to_string_lossy().into()
}

fn skill_source(entry: &Spec) -> &str {
    match entry {
        Spec::Skill { source, .. } => source,
        other => panic!("not a skill: {other:?}"),
    }
}

#[test]
fn before_a_sync_the_moved_skill_is_listed_as_stale() {
    let sb = moved_sandbox();

    let rows = sb.json(&["library", "list", "--json"]);

    assert_eq!(rows[0]["stale"], true);
}

#[test]
fn sync_re_points_the_item_and_the_profile_instead_of_adding_a_duplicate() {
    let sb = moved_sandbox();

    let report = sb.json(&["sync", "--json"]);

    assert_eq!(report["added"].as_array().unwrap().len(), 0);
    let moved = &report["repointed"][0];
    assert_eq!(moved["to"], new_skills(&sb));
    assert_eq!(moved["profiles"][0], "mine");
    let library = sb.library();
    assert_eq!(library.items().len(), 1);
    assert_eq!(skill_source(&library.items()[0].entry), new_skills(&sb));
    assert_eq!(
        skill_source(&sb.profile("mine").entries[0]),
        new_skills(&sb)
    );
}

#[test]
fn after_the_sync_nothing_is_stale_and_a_second_sync_moves_nothing() {
    let sb = moved_sandbox();
    sb.json(&["sync", "--json"]);

    let rows = sb.json(&["library", "list", "--json"]);
    let again: Value = sb.json(&["sync", "--json"]);

    assert_eq!(rows[0]["stale"], false);
    assert!(again["repointed"].as_array().unwrap().is_empty());
}

#[test]
fn profile_status_marks_an_entry_whose_folder_is_gone_as_unavailable() {
    let sb = moved_sandbox();

    let rows = sb.json(&["profile", "status", "mine", "--json"]);

    assert_eq!(rows[0]["unavailable"].as_array().unwrap().len(), 1);
}

#[test]
fn applying_an_entry_whose_folder_is_gone_fails_without_running_npx() {
    let sb = moved_sandbox();

    let out = sb.skillz(&["use", "-y", "mine"]);

    assert!(!out.status.success());
    assert!(sb.npx_calls().is_empty());
}
