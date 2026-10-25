//! `skillz uninstall` and the list browser's remove action.

mod common;

use std::fs;

use common::*;
use skillz::app::prompt::{Choice, Picked, Prompter};
use skillz::app::tree::Node;
use skillz::cli::{Command, Deps, Filter, ListArgs, UninstallArgs};
use skillz_harness::model::Agent;

fn claude_hooks() -> Filter {
    Filter {
        hooks: true,
        agents: vec![Agent::Claude],
        ..Filter::default()
    }
}

fn uninstall_with(w: &World, cmd: Command, prompter: &Scripted) -> anyhow::Result<()> {
    run_quiet(cmd, &deps(w, &Recorder::default(), prompter))
}

fn claude_settings_text(w: &World) -> String {
    fs::read_to_string(w.claude_settings()).unwrap()
}

fn after_removing_rtk_hooks() -> World {
    let w = world();
    uninstall_with(&w, uninstall("rtk", claude_hooks(), true), &Scripted::yes()).unwrap();
    w
}

fn after_removing_tdd() -> World {
    let w = world();
    uninstall_with(
        &w,
        uninstall("tdd", Filter::default(), true),
        &Scripted::yes(),
    )
    .unwrap();
    w
}

#[test]
fn removing_a_hook_tool_drops_every_group_that_held_only_its_handlers() {
    let w = after_removing_rtk_hooks();

    let settings = read_json(&w.claude_settings());

    assert!(settings["hooks"].get("PreToolUse").is_none());
}

#[test]
fn removing_a_hook_tool_keeps_other_handlers_of_a_shared_group() {
    let w = after_removing_rtk_hooks();

    let settings = read_json(&w.claude_settings());

    let session = settings["hooks"]["SessionStart"][0]["hooks"]
        .as_array()
        .unwrap();
    assert_eq!(session.len(), 1);
    assert_eq!(session[0]["command"], "bd prime");
}

#[test]
fn removing_a_hook_tool_keeps_unrelated_settings() {
    let w = after_removing_rtk_hooks();

    assert_eq!(read_json(&w.claude_settings())["model"], "opus");
}

#[test]
fn removing_a_hook_tool_leaves_other_agents_hooks() {
    let w = after_removing_rtk_hooks();

    assert!(read_json(&w.codex_global_hooks())["hooks"]["SessionStart"].is_array());
}

#[test]
fn removing_a_skill_deletes_its_store() {
    let w = after_removing_tdd();

    assert!(!w.tdd_store().exists());
}

#[test]
fn removing_a_skill_deletes_its_links_in_every_agent() {
    let w = after_removing_tdd();

    assert!(!present(w.home.join(".claude/skills/tdd")));
    assert!(!present(w.home.join(".codex/skills/tdd")));
}

#[test]
fn removing_a_skill_drops_its_lock_entry() {
    let w = after_removing_tdd();

    let lock = read_json(&w.home.join(".agents/.skill-lock.json"));

    assert!(lock["skills"].get("tdd").is_none());
}

#[test]
fn yes_removes_the_one_exact_match_without_asking_the_picker() {
    let w = world();

    uninstall_with(
        &w,
        uninstall("tdd", Filter::default(), true),
        &Scripted::picking(vec![]),
    )
    .unwrap();

    assert!(!w.tdd_store().join("SKILL.md").exists());
}

#[test]
fn yes_refuses_a_query_matching_several_items() {
    let w = world();

    // "bd" exists for three agents.
    let res = uninstall_with(
        &w,
        uninstall("bd", Filter::default(), true),
        &Scripted::yes(),
    );

    assert!(res.is_err());
}

#[test]
fn a_refused_ambiguous_query_changes_nothing() {
    let w = world();
    let before = claude_settings_text(&w);

    let _ = uninstall_with(
        &w,
        uninstall("bd", Filter::default(), true),
        &Scripted::yes(),
    );

    assert_eq!(claude_settings_text(&w), before);
    assert!(w.codex_project_hooks().exists());
}

#[test]
fn yes_refuses_a_single_inexact_match() {
    let w = world();

    let res = uninstall_with(
        &w,
        uninstall("tdd-old", Filter::default(), true),
        &Scripted::yes(),
    );

    assert!(res.is_err());
}

#[test]
fn a_refused_inexact_match_removes_nothing() {
    let w = world();

    let _ = uninstall_with(
        &w,
        uninstall("tdd-old", Filter::default(), true),
        &Scripted::yes(),
    );

    assert!(w.tdd_store().join("SKILL.md").exists());
}

#[test]
fn declining_the_confirmation_changes_nothing() {
    let w = world();
    let before = claude_settings_text(&w);

    uninstall_with(
        &w,
        uninstall("bd", Filter::default(), false),
        &Scripted::declining(),
    )
    .unwrap();

    assert_eq!(claude_settings_text(&w), before);
    assert!(w.codex_project_hooks().exists());
}

#[test]
fn picking_nothing_in_the_picker_removes_nothing() {
    let w = world();
    let before = claude_settings_text(&w);

    uninstall_with(
        &w,
        uninstall("bd", Filter::default(), false),
        &Scripted::picking(vec![]),
    )
    .unwrap();

    assert_eq!(claude_settings_text(&w), before);
}

#[test]
fn a_project_path_limits_removal_to_that_project() {
    let w = world();
    let cmd = Command::Uninstall(UninstallArgs {
        queries: vec!["bd".into()],
        project: Some(w.project.clone()),
        filter: Filter::default(),
        yes: true,
        dry_run: false,
    });

    // Only the codex project hook matches, so --yes is unambiguous.
    uninstall_with(&w, cmd, &Scripted::yes()).unwrap();

    let codex = read_json(&w.codex_project_hooks());
    assert_eq!(codex["hooks"].as_object().unwrap().len(), 0);
}

#[test]
fn a_project_path_leaves_global_hooks() {
    let w = world();
    let cmd = Command::Uninstall(UninstallArgs {
        queries: vec!["bd".into()],
        project: Some(w.project.clone()),
        filter: Filter::default(),
        yes: true,
        dry_run: false,
    });

    uninstall_with(&w, cmd, &Scripted::yes()).unwrap();

    assert!(read_json(&w.codex_global_hooks())["hooks"]["SessionStart"].is_array());
}

#[test]
fn a_dry_run_changes_nothing() {
    let w = world();
    let cmd = Command::Uninstall(UninstallArgs {
        queries: vec!["tdd".into()],
        project: None,
        filter: Filter::default(),
        yes: true,
        dry_run: true,
    });

    uninstall_with(&w, cmd, &Scripted::yes()).unwrap();

    assert!(w.tdd_store().join("SKILL.md").exists());
}

/// Browser that removes the given group indices (display order, as `tree::build`).
struct RemoveGroups(Vec<usize>);

impl Prompter for RemoveGroups {
    fn multi_select(&self, _title: &str, _options: &[String]) -> anyhow::Result<Vec<usize>> {
        unreachable!("browse is used instead")
    }
    fn confirm(&self, _q: &str) -> anyhow::Result<bool> {
        Ok(true)
    }
    fn browse(&self, _title: &str, _groups: &[Node]) -> anyhow::Result<Option<(Choice, Picked)>> {
        let picked = Picked {
            groups: self.0.clone(),
            items: vec![],
        };
        Ok(Some((Choice::Remove, picked)))
    }
}

/// Removes the skills group in the interactive browser. Its group is found by index; the
/// world also has hooks groups, the project one first.
fn after_browser_removes_the_skills_group() -> World {
    let w = world();
    let runner = Recorder::default();
    let prompter = RemoveGroups(vec![1]);
    let d = Deps {
        interactive: true,
        checker: std::sync::Arc::new(skillz_harness::update::check::NoCheck),
        scanner: None,
        ..deps(&w, &runner, &prompter)
    };
    let cmd = Command::List(ListArgs {
        target: None,
        project: None,
        all: false,
        json: false,
        check: false,
        available: false,
        filter: Filter::default(),
        interactive: true,
    });
    run_quiet(cmd, &d).unwrap();
    w
}

#[test]
fn the_browser_removes_the_marked_group_everywhere() {
    let w = after_browser_removes_the_skills_group();

    assert!(!w.tdd_store().exists());
    assert!(!present(w.home.join(".claude/skills/tdd")));
}

#[test]
fn the_browser_leaves_unmarked_groups() {
    let w = after_browser_removes_the_skills_group();

    let hooks = read_json(&w.claude_settings());

    assert!(!hooks["hooks"].as_object().unwrap().is_empty());
}

fn uninstall_each(queries: &[&str]) -> Command {
    Command::Uninstall(UninstallArgs {
        queries: queries.iter().map(|q| q.to_string()).collect(),
        project: None,
        filter: global(),
        yes: true,
        dry_run: false,
    })
}

#[test]
fn several_queries_remove_all_their_items_in_one_run() {
    let w = world();

    uninstall_with(&w, uninstall_each(&["tdd", "rtk"]), &Scripted::yes()).unwrap();

    // Assert: the skill and the hook tool are both gone
    assert!(!w.tdd_store().exists());
    assert!(
        read_json(&w.claude_settings())["hooks"]
            .get("PreToolUse")
            .is_none()
    );
}

#[test]
fn one_unmatched_query_among_several_removes_nothing() {
    let w = world();

    let result = uninstall_with(
        &w,
        uninstall_each(&["tdd", "zzz-nothing"]),
        &Scripted::yes(),
    );

    assert!(result.is_err());
    assert!(w.tdd_store().exists());
}
