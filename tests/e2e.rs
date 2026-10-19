//! End-to-end flows over a fake home + project, with real adapters and scripted prompts.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use skillz::agents;
use skillz::cli::{Command, Deps, Filter, run};
use skillz::model::{Agent, Kind, Scope};
use skillz::prompt::Prompter;
use skillz::runner::{CommandRunner, Output};

struct Scripted {
    pick: Option<Vec<usize>>,
    confirm: bool,
}

impl Prompter for Scripted {
    fn multi_select(&self, _title: &str, options: &[String]) -> anyhow::Result<Vec<usize>> {
        Ok(self
            .pick
            .clone()
            .unwrap_or_else(|| (0..options.len()).collect()))
    }
    fn confirm(&self, _q: &str) -> anyhow::Result<bool> {
        Ok(self.confirm)
    }
}

#[derive(Default)]
struct Recorder(RefCell<Vec<(String, Vec<String>)>>);

impl CommandRunner for Recorder {
    fn run(&self, program: &str, args: &[String], _cwd: &Path) -> anyhow::Result<Output> {
        self.0.borrow_mut().push((program.into(), args.to_vec()));
        Ok(Output {
            success: true,
            ..Default::default()
        })
    }
}

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

struct World {
    _tmp: tempfile::TempDir,
    home: PathBuf,
    project: PathBuf,
}

/// Global: an npx skill linked into Claude + Cursor, two hook tools in Claude settings,
/// a Cursor hook. Project: a Codex hook.
fn world() -> World {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let project = tmp.path().join("proj");
    fs::create_dir_all(&project).unwrap();

    write(
        &home.join(".agents/skills/tdd/SKILL.md"),
        "---\nname: tdd\n---\n",
    );
    write(
        &home.join(".agents/.skill-lock.json"),
        &json!({"version": 3, "skills": {"tdd": {"source": "mattpocock/skills"}}}).to_string(),
    );
    for agent_dir in [".claude/skills", ".cursor/skills"] {
        fs::create_dir_all(home.join(agent_dir)).unwrap();
        std::os::unix::fs::symlink(
            home.join(".agents/skills/tdd"),
            home.join(agent_dir).join("tdd"),
        )
        .unwrap();
    }
    write(
        &home.join(".claude/settings.json"),
        &json!({
            "model": "opus",
            "hooks": {
                "PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "rtk hook claude"}]}],
                "SessionStart": [{"matcher": "", "hooks": [
                    {"type": "command", "command": "bd prime"},
                    {"type": "command", "command": "rtk hook session"}
                ]}]
            }
        })
        .to_string(),
    );
    write(
        &home.join(".cursor/hooks.json"),
        &json!({"version": 1, "hooks": {"sessionStart": [{"command": "bd cursor-hook sessionStart"}]}}).to_string(),
    );
    write(
        &project.join(".codex/hooks.json"),
        &json!({"hooks": {"SessionStart": [{"hooks": [{"type": "command", "command": "bd codex-hook SessionStart"}]}]}}).to_string(),
    );
    World {
        _tmp: tmp,
        home,
        project,
    }
}

fn deps<'a>(w: &World, runner: &'a Recorder, prompter: &'a Scripted) -> Deps<'a> {
    Deps {
        home: w.home.clone(),
        cwd: w.project.clone(),
        adapters: agents::registry(),
        runner,
        prompter,
        interactive: false,
    }
}

fn uninstall(query: &str, filter: Filter, yes: bool) -> Command {
    Command::Uninstall {
        query: Some(query.into()),
        project: None,
        filter,
        yes,
        dry_run: false,
    }
}

#[test]
fn inventory_finds_everything_once() {
    let w = world();
    let env = skillz::env::Env {
        home: w.home.canonicalize().unwrap(),
        project: w.project.canonicalize().unwrap(),
    };
    let inv = agents::inventory(&agents::registry(), &env, &[Scope::Global, Scope::Project]);
    assert!(inv.warnings.is_empty(), "{:?}", inv.warnings);

    let skills: Vec<_> = inv.items.iter().filter(|i| i.kind == Kind::Skill).collect();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "tdd");
    assert!(skills[0].agents.contains(&Agent::Claude));
    assert!(skills[0].agents.contains(&Agent::Cursor));

    let hook_names: Vec<(&str, Agent, Scope)> = inv
        .items
        .iter()
        .filter(|i| i.kind == Kind::Hook)
        .map(|i| (i.name.as_str(), i.agents[0], i.scope))
        .collect();
    assert_eq!(hook_names.len(), 4);
    assert!(hook_names.contains(&("rtk", Agent::Claude, Scope::Global)));
    assert!(hook_names.contains(&("bd", Agent::Claude, Scope::Global)));
    assert!(hook_names.contains(&("bd", Agent::Cursor, Scope::Global)));
    assert!(hook_names.contains(&("bd", Agent::Codex, Scope::Project)));
}

#[test]
fn list_renders_without_error() {
    let w = world();
    let (r, p) = (
        Recorder::default(),
        Scripted {
            pick: None,
            confirm: true,
        },
    );
    let mut out = Vec::new();
    let cmd = Command::List {
        path: None,
        filter: Filter::default(),
        interactive: false,
    };
    run(cmd, &deps(&w, &r, &p), &mut out).unwrap();
    assert!(!out.is_empty());
}

#[test]
fn uninstall_hook_group_removes_all_its_handlers_and_keeps_others() {
    let w = world();
    let (r, p) = (
        Recorder::default(),
        Scripted {
            pick: None,
            confirm: true,
        },
    );
    let filter = Filter {
        hooks: true,
        agents: vec![Agent::Claude],
        ..Filter::default()
    };
    run(
        uninstall("rtk", filter, true),
        &deps(&w, &r, &p),
        &mut Vec::new(),
    )
    .unwrap();

    let settings = read_json(&w.home.join(".claude/settings.json"));
    assert_eq!(settings["model"], "opus");
    assert!(settings["hooks"].get("PreToolUse").is_none());
    let session = settings["hooks"]["SessionStart"][0]["hooks"]
        .as_array()
        .unwrap();
    assert_eq!(session.len(), 1);
    assert_eq!(session[0]["command"], "bd prime");
    // Other agents' bd hooks untouched.
    assert!(read_json(&w.home.join(".cursor/hooks.json"))["hooks"]["sessionStart"].is_array());
}

#[test]
fn uninstall_skill_removes_links_store_and_lock_entry() {
    let w = world();
    let (r, p) = (
        Recorder::default(),
        Scripted {
            pick: None,
            confirm: true,
        },
    );
    run(
        uninstall("tdd", Filter::default(), true),
        &deps(&w, &r, &p),
        &mut Vec::new(),
    )
    .unwrap();

    assert!(!w.home.join(".agents/skills/tdd").exists());
    assert!(fs::symlink_metadata(w.home.join(".claude/skills/tdd")).is_err());
    assert!(fs::symlink_metadata(w.home.join(".cursor/skills/tdd")).is_err());
    let lock = read_json(&w.home.join(".agents/.skill-lock.json"));
    assert!(lock["skills"].get("tdd").is_none());
}

#[test]
fn yes_refuses_ambiguous_matches() {
    let w = world();
    let (r, p) = (
        Recorder::default(),
        Scripted {
            pick: None,
            confirm: true,
        },
    );
    // "bd" exists for three agents.
    let res = run(
        uninstall("bd", Filter::default(), true),
        &deps(&w, &r, &p),
        &mut Vec::new(),
    );
    assert!(res.is_err());
    assert!(read_json(&w.home.join(".cursor/hooks.json"))["hooks"]["sessionStart"].is_array());
}

#[test]
fn declining_confirmation_changes_nothing() {
    let w = world();
    let before = fs::read_to_string(w.home.join(".claude/settings.json")).unwrap();
    let (r, p) = (
        Recorder::default(),
        Scripted {
            pick: None,
            confirm: false,
        },
    );
    run(
        uninstall("bd", Filter::default(), false),
        &deps(&w, &r, &p),
        &mut Vec::new(),
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(w.home.join(".claude/settings.json")).unwrap(),
        before
    );
    assert!(w.project.join(".codex/hooks.json").exists());
}

#[test]
fn picker_selection_limits_removal() {
    let w = world();
    let (r, p) = (
        Recorder::default(),
        Scripted {
            pick: Some(vec![]),
            confirm: true,
        },
    );
    let before = fs::read_to_string(w.home.join(".claude/settings.json")).unwrap();
    run(
        uninstall("bd", Filter::default(), false),
        &deps(&w, &r, &p),
        &mut Vec::new(),
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(w.home.join(".claude/settings.json")).unwrap(),
        before
    );
}

#[test]
fn project_path_scopes_to_project_only() {
    let w = world();
    let (r, p) = (
        Recorder::default(),
        Scripted {
            pick: None,
            confirm: true,
        },
    );
    let filter = Filter::default();
    let cmd = Command::Uninstall {
        query: Some("bd".into()),
        project: Some(w.project.clone()),
        filter,
        yes: true,
        dry_run: false,
    };
    // Only the codex project hook matches, so --yes is unambiguous.
    run(cmd, &deps(&w, &r, &p), &mut Vec::new()).unwrap();
    let codex = read_json(&w.project.join(".codex/hooks.json"));
    assert_eq!(codex["hooks"].as_object().unwrap().len(), 0);
    assert!(read_json(&w.home.join(".cursor/hooks.json"))["hooks"]["sessionStart"].is_array());
}

#[test]
fn dry_run_changes_nothing() {
    let w = world();
    let (r, p) = (
        Recorder::default(),
        Scripted {
            pick: None,
            confirm: true,
        },
    );
    let cmd = Command::Uninstall {
        query: Some("tdd".into()),
        project: None,
        filter: Filter::default(),
        yes: true,
        dry_run: true,
    };
    run(cmd, &deps(&w, &r, &p), &mut Vec::new()).unwrap();
    assert!(w.home.join(".agents/skills/tdd/SKILL.md").exists());
}

#[test]
fn install_local_source_runs_npx_skills_with_matches() {
    let w = world();
    let src = w.project.join("src-repo");
    write(&src.join("skills/git-commit/SKILL.md"), "x");
    write(&src.join("skills/caveman-commit/SKILL.md"), "x");
    write(&src.join("skills/tdd/SKILL.md"), "x");
    let (r, p) = (
        Recorder::default(),
        Scripted {
            pick: None,
            confirm: true,
        },
    );
    let cmd = Command::Install {
        source: src.display().to_string(),
        global: true,
        agents: vec![Agent::Claude],
        all: false,
        skill: vec!["*commit".into()],
        project: None,
        yes: true,
    };
    run(cmd, &deps(&w, &r, &p), &mut Vec::new()).unwrap();
    let calls = r.0.borrow();
    let (program, args) = calls.last().unwrap();
    assert_eq!(program, "npx");
    let skills: Vec<&String> = args
        .iter()
        .zip(args.iter().skip(1))
        .filter(|(flag, _)| *flag == "--skill")
        .map(|(_, v)| v)
        .collect();
    assert_eq!(skills.len(), 2);
    assert!(!skills.iter().any(|s| *s == "tdd"));
    assert!(args.contains(&"-g".to_string()));
}
