//! Subagent definitions copied in by a profile and removed by an exact apply.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use common::*;
use serde_json::Value;
use skillz::cli::{Command, Deps, Filter, ListArgs};

const TEAM_PROFILE: &str = "agents = [\"claude\"]\n[[entry]]\nkind = \"subagent\"\nsource = \"agents-src\"\nnames = [\"frontend-developer\", \"ui\"]\n";

/// A local repo with two definitions, and a `team` profile that copies both for Claude.
fn world_with_team_profile() -> (World, PathBuf) {
    let w = world();
    let src = w.project.join("agents-src");
    for (rel, name) in [
        ("engineering/eng-frontend.md", "Frontend Developer"),
        ("design/ui.md", "UI Designer"),
    ] {
        write(
            &src.join(rel),
            &format!("---\nname: {name}\ndescription: d\n---\nbody\n"),
        );
    }
    w.write_profile("team", TEAM_PROFILE);
    let agents = w.project.join(".claude/agents");
    (w, agents)
}

fn use_team(d: &Deps) {
    run_quiet(use_cmd("team", false), d).unwrap();
}

fn unload(w: &World, d: &Deps) {
    w.write_profile("empty", "");
    run_quiet(use_exact_cmd("empty"), d).unwrap();
}

fn manifest(agents: &Path) -> BTreeMap<String, String> {
    serde_json::from_str(&fs::read_to_string(agents.join(".skillz-sources.json")).unwrap()).unwrap()
}

fn listed_subagents(d: &Deps) -> Vec<Value> {
    let cmd = Command::List(ListArgs {
        target: None,
        project: Some(PathBuf::from(".")),
        all: true,
        json: true,
        filter: Filter {
            subagents: true,
            ..Filter::default()
        },
        interactive: false,
    });
    serde_json::from_slice::<Value>(&run_captured(cmd, d))
        .unwrap()
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn using_a_profile_copies_the_named_definitions_in() {
    let (w, agents) = world_with_team_profile();
    let (r, p) = (Recorder::default(), Scripted::yes());

    use_team(&deps(&w, &r, &p));

    assert!(agents.join("eng-frontend.md").exists());
    assert!(agents.join("ui.md").exists());
}

#[test]
fn a_local_source_needs_no_commands() {
    let (w, _agents) = world_with_team_profile();
    let (r, p) = (Recorder::default(), Scripted::yes());

    use_team(&deps(&w, &r, &p));

    assert_eq!(r.count(), 0);
}

#[test]
fn copied_definitions_are_listed_with_where_they_came_from() {
    let (w, _agents) = world_with_team_profile();
    let (r, p) = (Recorder::default(), Scripted::yes());
    let d = deps(&w, &r, &p);
    use_team(&d);

    let items = listed_subagents(&d);

    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|i| i["source"] == "agents-src"));
}

#[test]
fn an_exact_empty_profile_removes_the_copied_files() {
    let (w, agents) = world_with_team_profile();
    let (r, p) = (Recorder::default(), Scripted::yes());
    let d = deps(&w, &r, &p);
    use_team(&d);

    unload(&w, &d);

    assert!(!agents.join("eng-frontend.md").exists());
    assert!(!agents.join("ui.md").exists());
}

#[test]
fn removing_the_copied_files_forgets_their_sources() {
    let (w, agents) = world_with_team_profile();
    let (r, p) = (Recorder::default(), Scripted::yes());
    let d = deps(&w, &r, &p);
    use_team(&d);

    unload(&w, &d);

    assert!(!manifest(&agents).contains_key("ui.md"));
}

#[test]
fn a_file_that_was_already_there_makes_the_copy_fail() {
    let (w, agents) = world_with_team_profile();
    write(&agents.join("ui.md"), "mine");
    let (r, p) = (Recorder::default(), Scripted::yes());

    let res = run_quiet(use_cmd("team", false), &deps(&w, &r, &p));

    assert!(res.is_err());
}

#[test]
fn a_file_that_was_already_there_is_never_overwritten() {
    let (w, agents) = world_with_team_profile();
    write(&agents.join("ui.md"), "mine");
    let (r, p) = (Recorder::default(), Scripted::yes());

    let _ = run_quiet(use_cmd("team", false), &deps(&w, &r, &p));

    assert_eq!(fs::read_to_string(agents.join("ui.md")).unwrap(), "mine");
}
