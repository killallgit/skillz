//! A skill and a subagent present for both Claude and Codex are one logical item.

mod common;

use common::*;
use serde_json::Value;

const SKILL: &str = "---\nname: tdd\ndescription: d\n---\n";
const CLAUDE_AGENT: &str = "---\nname: reviewer\ndescription: d\n---\nbody\n";
const CODEX_AGENT: &str = "name = \"reviewer\"\ndescription = \"d\"\n";

/// A project where `tdd` and `reviewer` exist for Claude and for Codex.
fn sandbox_with_both_agents() -> Sandbox {
    let sb = Sandbox::new();
    write(&sb.project.join(".claude/skills/tdd/SKILL.md"), SKILL);
    write(&sb.project.join(".codex/skills/tdd/SKILL.md"), SKILL);
    write(&sb.project.join(".claude/agents/reviewer.md"), CLAUDE_AGENT);
    write(&sb.project.join(".codex/agents/reviewer.toml"), CODEX_AGENT);
    sb
}

fn rows_named<'a>(listed: &'a Value, name: &str) -> Vec<&'a Value> {
    listed
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["name"] == name)
        .collect()
}

fn copies_exist(sb: &Sandbox) -> [bool; 4] {
    [
        present(sb.project.join(".claude/skills/tdd")),
        present(sb.project.join(".codex/skills/tdd")),
        present(sb.project.join(".claude/agents/reviewer.md")),
        present(sb.project.join(".codex/agents/reviewer.toml")),
    ]
}

#[test]
fn json_has_one_row_per_logical_item_that_lists_its_copies() {
    let sb = sandbox_with_both_agents();

    let listed = sb.json(&["list", "--json"]);

    for name in ["tdd", "reviewer"] {
        let rows = rows_named(&listed, name);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["merged"], true);
        assert_eq!(rows[0]["agents"], serde_json::json!(["claude", "codex"]));
        assert_eq!(rows[0]["members"].as_array().unwrap().len(), 2);
    }
}

#[test]
fn json_names_the_package_of_every_item() {
    let sb = sandbox_with_both_agents();

    let listed = sb.json(&["list", "--json"]);

    let tdd = rows_named(&listed, "tdd")[0]["package"].clone();
    assert_eq!(tdd, rows_named(&listed, "reviewer")[0]["package"]);
    assert!(tdd.is_string());
}

#[test]
fn uninstalling_by_name_is_one_match_and_removes_every_copy() {
    let sb = sandbox_with_both_agents();

    let out = sb.skillz(&["uninstall", "reviewer", "tdd", "--yes"]);

    assert!(out.status.success());
    assert_eq!(copies_exist(&sb), [false; 4]);
}

#[test]
fn an_agent_filter_acts_on_that_agents_copy_only() {
    let sb = sandbox_with_both_agents();

    let out = sb.skillz(&["uninstall", "reviewer", "--yes", "-a", "claude"]);

    assert!(out.status.success());
    assert_eq!(copies_exist(&sb), [true, true, false, true]);
}

#[test]
fn a_dry_run_plans_the_removal_of_every_copy_and_removes_nothing() {
    let sb = sandbox_with_both_agents();

    let out = sb.skillz(&["uninstall", "reviewer", "--dry-run"]);

    assert!(out.status.success());
    assert_eq!(copies_exist(&sb), [true; 4]);
}
