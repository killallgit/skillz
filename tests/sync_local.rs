//! `skillz sync` through the binary: hand-made skills and subagents reach the library by the
//! directory holding them.

mod common;

use std::fs;
use std::path::Path;

use common::*;
use serde_json::Value;

const SKILL: &str = "---\nname: NAME\ndescription: d\n---\n";

fn hand_made_skill(dir: &Path, name: &str) {
    write(
        &dir.join(name).join("SKILL.md"),
        &SKILL.replace("NAME", name),
    );
}

fn sandbox_with_hand_made_items() -> Sandbox {
    let sb = Sandbox::new();
    hand_made_skill(&sb.project.join(".agents/skills"), "beads");
    hand_made_skill(&sb.claude.join("skills"), "mine");
    write(
        &sb.project.join(".claude/agents/rev.md"),
        "---\nname: rev\n---\n",
    );
    sb
}

fn sync(sb: &Sandbox) -> Value {
    let out = sb.skillz(&["sync", "--json"]);
    assert!(out.status.success());
    serde_json::from_slice(&out.stdout).unwrap()
}

fn added(report: &Value, kind: &str) -> Vec<Value> {
    report["added"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["kind"] == kind)
        .cloned()
        .collect()
}

fn link_of(item: &Value) -> &str {
    item["link"].as_str().unwrap()
}

#[test]
fn hand_made_skills_at_both_scopes_are_recorded_by_the_dir_holding_them() {
    let sb = sandbox_with_hand_made_items();

    let report = sync(&sb);

    // Assert each skill was added with the manual engine and its directory as the link.
    let skills = added(&report, "skill");
    let mut links: Vec<&str> = skills.iter().map(link_of).collect();
    links.sort();
    let project = sb.project.join(".agents/skills");
    let global = sb.claude.join("skills");
    let mut expected = [project.to_str().unwrap(), global.to_str().unwrap()];
    expected.sort();
    assert_eq!(links, expected);
    assert!(skills.iter().all(|item| item["engine"] == "local"));
}

#[test]
fn a_hand_made_subagent_is_recorded_by_its_directory() {
    let sb = sandbox_with_hand_made_items();

    let report = sync(&sb);

    let subagents = added(&report, "subagent");
    let agents_dir = sb.project.join(".claude/agents");
    assert_eq!(subagents.len(), 1);
    assert_eq!(link_of(&subagents[0]), agents_dir.to_str().unwrap());
}

#[test]
fn nothing_hand_made_is_reported_as_skipped() {
    let sb = sandbox_with_hand_made_items();

    let report = sync(&sb);

    assert!(report["skipped"].as_array().unwrap().is_empty());
}

#[test]
fn the_library_stores_them_as_detected_and_a_second_sync_adds_nothing() {
    let sb = sandbox_with_hand_made_items();
    sync(&sb);
    let store = sb.skillz_dir().join("store.json");
    let first = fs::read_to_string(&store).unwrap();

    let again = sync(&sb);

    // Assert the stored copies say they were detected and nothing changed on disk.
    let stored: Value = serde_json::from_str(&first).unwrap();
    let items = stored["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);
    assert!(items.iter().all(|i| i["provenance"]["detected"] == true));
    assert!(again["added"].as_array().unwrap().is_empty());
    assert_eq!(fs::read_to_string(&store).unwrap(), first);
}
