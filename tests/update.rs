//! `skillz update` and `skillz update --check`.

mod common;

use std::fs;
use std::sync::Arc;

use common::*;
use skillz::cli::{Command, Deps, Filter, UpdateArgs};
use skillz_harness::model::Installed;
use skillz_harness::update::check::UpdateCheck;

/// A project with `tdd` installed the way `npx skills` leaves it, locked or not.
fn world_with_skill(locked: bool) -> World {
    let tmp = tempfile::tempdir().unwrap();
    let (home, project) = (tmp.path().join("home"), tmp.path().join("proj"));
    let store = project.join(".agents/skills/tdd");
    write(
        &store.join("SKILL.md"),
        "---\nname: tdd\ndescription: d\n---\n",
    );
    fs::create_dir_all(project.join(".claude/skills")).unwrap();
    std::os::unix::fs::symlink(&store, project.join(".claude/skills/tdd")).unwrap();
    if locked {
        let lock = r#"{"skills":{"tdd":{"source":"o/r","computedHash":"old"}}}"#;
        write(&project.join("skills-lock.json"), lock);
    }
    fs::create_dir_all(&home).unwrap();
    World {
        _tmp: tmp,
        home,
        project,
    }
}

fn update_command(query: Option<&str>, dry_run: bool) -> Command {
    Command::Update(UpdateArgs {
        target: query.map(Into::into),
        project: None,
        filter: Filter::default(),
        yes: true,
        dry_run,
        check: false,
        json: false,
    })
}

fn check_command(json: bool) -> Command {
    Command::Update(UpdateArgs {
        check: true,
        json,
        ..match update_command(None, false) {
            Command::Update(args) => args,
            _ => unreachable!(),
        }
    })
}

struct Outdated(&'static str);

impl UpdateCheck for Outdated {
    fn available(&self, item: &Installed) -> bool {
        item.name == self.0
    }
}

fn update_with(w: &World, command: Command) -> (anyhow::Result<i32>, Recorder) {
    let runner = Recorder::default();
    let result = skillz::cli::run(
        command,
        &deps(w, &runner, &Scripted::yes()),
        &mut Vec::new(),
    );
    (result, runner)
}

#[test]
fn updating_a_skill_runs_the_update_of_npx_skills_in_the_project() {
    let w = world_with_skill(true);

    let (result, runner) = update_with(&w, update_command(Some("tdd"), false));

    let calls = runner.calls();
    assert!(result.is_ok());
    assert_eq!(calls.len(), 1);
    assert!(calls[0].1.contains(&"update".to_string()));
    assert!(calls[0].1.contains(&"tdd".to_string()));
}

#[test]
fn a_dry_run_changes_nothing() {
    let w = world_with_skill(true);

    let (result, runner) = update_with(&w, update_command(Some("tdd"), true));

    assert!(result.is_ok());
    assert_eq!(runner.count(), 0);
}

#[test]
fn a_skill_nothing_installed_has_no_update_to_run() {
    let w = world_with_skill(false);

    let (result, runner) = update_with(&w, update_command(Some("tdd"), false));

    assert!(result.is_err());
    assert_eq!(runner.count(), 0);
}

fn check_output(w: &World, json: bool, checker: &'static str) -> (String, Recorder) {
    let runner = Recorder::default();
    let prompter = Scripted::yes();
    let d = Deps {
        checker: Arc::new(Outdated(checker)),
        scanner: None,
        ..deps(w, &runner, &prompter)
    };
    let mut out = Vec::new();
    skillz::cli::run(check_command(json), &d, &mut out).unwrap();
    (String::from_utf8(out).unwrap(), runner)
}

#[test]
fn check_lists_the_items_the_checker_finds_outdated_as_json() {
    let w = world_with_skill(true);

    let (out, runner) = check_output(&w, true, "tdd");

    let rows: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(rows[0]["name"], "tdd");
    assert_eq!(rows.as_array().unwrap().len(), 1);
    assert_eq!(runner.count(), 0);
}

#[test]
fn check_lists_nothing_when_nothing_is_outdated() {
    let w = world_with_skill(true);

    let (out, _) = check_output(&w, true, "other");

    let rows: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(rows.as_array().unwrap().is_empty());
}

#[test]
fn an_agent_filter_updates_only_the_selected_copy_of_a_merged_subagent() {
    use skillz_harness::model::Agent;
    for selected in [Agent::Claude, Agent::Codex] {
        let w = world_with_skill(false);
        let source = w.project.join("definitions");
        let md = "---\nname: reviewer\n---\nnew";
        let toml = "name = \"reviewer\"\nbody = \"new\"\n";
        write(&source.join("reviewer.md"), md);
        write(&source.join("reviewer.toml"), toml);
        for (agent, ext, old) in [
            ("claude", "md", "---\nname: reviewer\n---\nold"),
            ("codex", "toml", "name = \"reviewer\"\nbody = \"old\"\n"),
        ] {
            let dir = w.project.join(format!(".{agent}/agents"));
            write(&dir.join(format!("reviewer.{ext}")), old);
            write(
                &dir.join(".skillz-sources.json"),
                &serde_json::json!({format!("reviewer.{ext}"): source}).to_string(),
            );
        }
        let mut command = match update_command(Some("reviewer"), false) {
            Command::Update(args) => args,
            _ => unreachable!(),
        };
        command.filter.agents = vec![selected];

        let (result, _) = update_with(&w, Command::Update(command));

        assert!(result.is_ok());
        let claude = fs::read_to_string(w.project.join(".claude/agents/reviewer.md")).unwrap();
        let codex = fs::read_to_string(w.project.join(".codex/agents/reviewer.toml")).unwrap();
        assert_eq!(claude == md, selected == Agent::Claude);
        assert_eq!(codex == toml, selected == Agent::Codex);
    }
}
