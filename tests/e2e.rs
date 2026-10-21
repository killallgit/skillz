//! End-to-end flows over a fake home + project, with real adapters and scripted prompts.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use skillz::action::{Action, execute};
use skillz::agents;
use skillz::cli::{Command, Deps, Filter, run};
use skillz::model::{Agent, Augmentation, Kind, Scope};
use skillz::prompt::{Choice, Picked, Prompter};
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
        claude_config: None,
        codex_home: None,
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
        claude_config: None,
        codex_home: None,
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
fn project_link_into_global_store_is_its_own_item() {
    let w = world();
    let env = skillz::env::Env {
        home: w.home.canonicalize().unwrap(),
        project: w.project.canonicalize().unwrap(),
        claude_config: None,
        codex_home: None,
    };
    let proj_link = env.project.join(".claude/skills/tdd");
    fs::create_dir_all(proj_link.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(env.home.join(".agents/skills/tdd"), &proj_link).unwrap();
    let skills = |scopes: &[Scope]| -> Vec<Augmentation> {
        let inv = agents::inventory(&agents::registry(), &env, scopes);
        inv.items
            .into_iter()
            .filter(|i| i.kind == Kind::Skill)
            .collect()
    };
    let removed_paths = |a: &Augmentation| -> Vec<PathBuf> {
        a.removal
            .iter()
            .filter_map(|act| match act {
                Action::RemovePath(t) => Some(t.path().to_path_buf()),
                _ => None,
            })
            .collect()
    };

    let only_project = skills(&[Scope::Project]);
    assert_eq!(only_project.len(), 1);
    assert_eq!(only_project[0].scope, Scope::Project);
    assert_eq!(removed_paths(&only_project[0]), vec![proj_link.clone()]);

    let both = skills(&[Scope::Global, Scope::Project]);
    let scopes: Vec<Scope> = both.iter().map(|a| a.scope).collect();
    assert_eq!(scopes.len(), 2);
    assert!(scopes.contains(&Scope::Global) && scopes.contains(&Scope::Project));
    let plan: Vec<Action> = both.iter().flat_map(|a| a.removal.clone()).collect();
    execute(&plan, &skillz::runner::NoCommands).unwrap();
    assert!(fs::symlink_metadata(&proj_link).is_err());
    assert!(!env.home.join(".agents/skills/tdd").exists());
}

fn list_lines(w: &World, target: Option<&str>, all: bool) -> Vec<String> {
    let (r, p) = (
        Recorder::default(),
        Scripted {
            pick: None,
            confirm: true,
        },
    );
    let mut out = Vec::new();
    let cmd = Command::List {
        target: target.map(Into::into),
        project: None,
        all,
        json: false,
        filter: Filter::default(),
        interactive: false,
    };
    run(cmd, &deps(w, &r, &p), &mut out).unwrap();
    String::from_utf8(out)
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with("NAME"))
        .map(Into::into)
        .collect()
}

fn is_top_level(line: &str) -> bool {
    !line.starts_with(' ')
}

#[test]
fn list_collapses_to_group_headers_unless_all() {
    let w = world();
    let collapsed = list_lines(&w, None, false);
    let expanded = list_lines(&w, None, true);
    assert!(!collapsed.is_empty());
    assert!(collapsed.iter().all(|l| is_top_level(l)));
    assert_eq!(
        collapsed.len(),
        expanded.iter().filter(|l| is_top_level(l)).count()
    );
    assert!(expanded.len() > collapsed.len());
}

#[test]
fn list_query_expands_only_matches() {
    let w = world();
    let tdd = list_lines(&w, Some("tdd"), false);
    assert_eq!(tdd.len(), 2);
    assert!(is_top_level(&tdd[0]) && !is_top_level(&tdd[1]));
    assert_eq!(list_lines(&w, Some("zzzz-no-such-thing"), false).len(), 1);
}

#[test]
fn list_target_directory_is_the_project_not_a_query() {
    let w = world();
    let project_only = list_lines(&w, Some("."), false);
    let both = list_lines(&w, None, false);
    assert!(!project_only.is_empty());
    assert!(project_only.len() < both.len());
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
    for name in ["git-commit", "caveman-commit", "tdd"] {
        write(
            &src.join(format!("skills/{name}/SKILL.md")),
            &format!("---\nname: {name}\ndescription: d\n---\n"),
        );
    }
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

#[test]
fn install_relative_source_resolves_against_cwd_not_project_dir() {
    let w = world();
    write(
        &w.project.join("src-repo/skills/tdd/SKILL.md"),
        "---\nname: tdd\ndescription: d\n---\n",
    );
    let other = w.project.parent().unwrap().join("other");
    fs::create_dir_all(&other).unwrap();
    let (r, p) = (
        Recorder::default(),
        Scripted {
            pick: None,
            confirm: true,
        },
    );
    let cmd = Command::Install {
        source: "./src-repo".into(),
        global: false,
        agents: vec![Agent::Codex],
        all: true,
        skill: vec![],
        project: Some("../other".into()),
        yes: true,
    };
    run(cmd, &deps(&w, &r, &p), &mut Vec::new()).unwrap();
    let calls = r.0.borrow();
    assert_eq!(calls.len(), 1);
    let (program, args) = &calls[0];
    assert_eq!(program, "npx");
    assert_eq!(
        Path::new(&args[3]).canonicalize().unwrap(),
        w.project.join("src-repo").canonicalize().unwrap()
    );
}

#[test]
fn install_from_home_is_global() {
    let w = world();
    let src = w.project.join("src-repo");
    write(
        &src.join("skills/tdd/SKILL.md"),
        "---\nname: tdd\ndescription: d\n---\n",
    );
    let (r, p) = (
        Recorder::default(),
        Scripted {
            pick: None,
            confirm: true,
        },
    );
    for project in [None, Some(w.home.clone())] {
        let cmd = Command::Install {
            source: src.display().to_string(),
            global: false,
            agents: vec![Agent::Codex],
            all: true,
            skill: vec![],
            project,
            yes: true,
        };
        let from_home = Deps {
            cwd: w.home.clone(),
            ..deps(&w, &r, &p)
        };
        run(cmd, &from_home, &mut Vec::new()).unwrap();
        let calls = r.0.borrow();
        let (program, args) = calls.last().unwrap();
        assert_eq!(program, "npx");
        assert!(args.contains(&"-g".to_string()));
    }
}

#[test]
fn yes_refuses_non_exact_single_match() {
    let w = world();
    let (r, p) = (
        Recorder::default(),
        Scripted {
            pick: None,
            confirm: true,
        },
    );
    let res = run(
        uninstall("tdd-old", Filter::default(), true),
        &deps(&w, &r, &p),
        &mut Vec::new(),
    );
    assert!(res.is_err());
    assert!(w.home.join(".agents/skills/tdd/SKILL.md").exists());
}

#[test]
fn install_rejects_option_like_source() {
    let w = world();
    let (r, p) = (
        Recorder::default(),
        Scripted {
            pick: None,
            confirm: true,
        },
    );
    let cmd = Command::Install {
        source: "--upload-pack=touch /tmp/x".into(),
        global: false,
        agents: vec![],
        all: true,
        skill: vec![],
        project: None,
        yes: true,
    };
    assert!(run(cmd, &deps(&w, &r, &p), &mut Vec::new()).is_err());
    assert!(r.0.borrow().is_empty());
}

fn list_json(w: &World, target: Option<&str>) -> Vec<Value> {
    let (r, p) = (
        Recorder::default(),
        Scripted {
            pick: None,
            confirm: true,
        },
    );
    let mut out = Vec::new();
    let cmd = Command::List {
        target: target.map(Into::into),
        project: None,
        all: false,
        json: true,
        filter: Filter::default(),
        interactive: false,
    };
    run(cmd, &deps(w, &r, &p), &mut out).unwrap();
    serde_json::from_slice::<Value>(&out)
        .unwrap()
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn list_json_dumps_every_item_with_its_removal_plan() {
    let w = world();
    let all = list_json(&w, None);
    let inv = agents::inventory(
        &agents::registry(),
        &skillz::env::Env {
            home: w.home.canonicalize().unwrap(),
            project: w.project.canonicalize().unwrap(),
            claude_config: None,
            codex_home: None,
        },
        &[Scope::Global, Scope::Project],
    );
    assert_eq!(all.len(), inv.items.len());
    let tdd: Vec<&Value> = all.iter().filter(|i| i["name"] == "tdd").collect();
    assert_eq!(tdd.len(), 1);
    assert_eq!(tdd[0]["kind"], "skill");
    assert_eq!(tdd[0]["scope"], "global");
    assert!(!tdd[0]["removal"].as_array().unwrap().is_empty());
    assert_eq!(list_json(&w, Some("tdd")).len(), 1);
    assert!(list_json(&w, Some("zzzz-no-such-thing")).is_empty());
}

fn list_items_picking(w: &World, query: &str, pick: Option<Vec<usize>>, json: bool) -> usize {
    let (r, p) = (
        Recorder::default(),
        Scripted {
            pick,
            confirm: true,
        },
    );
    let mut d = deps(w, &r, &p);
    d.interactive = true;
    let mut out = Vec::new();
    let cmd = Command::List {
        target: Some(query.into()),
        project: None,
        all: false,
        json,
        filter: Filter::default(),
        interactive: false,
    };
    run(cmd, &d, &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    if json {
        return serde_json::from_str::<Value>(&text)
            .unwrap()
            .as_array()
            .unwrap()
            .len();
    }
    // Items are the depth-1 rows: indented by two spaces, their details by four.
    text.lines()
        .filter(|l| l.starts_with("  ") && !l.starts_with("   "))
        .count()
}

#[test]
fn list_query_matching_several_groups_shows_only_the_picked_groups() {
    let w = world();
    // `bd` names three hooks (Claude, Codex, Cursor), one per "<agent> hooks" group.
    assert_eq!(list_items_picking(&w, "bd", Some(vec![0, 1, 2]), false), 3);
    // `s` hits the hooks and the `mattpocock/skills` skill: four groups, so pick some.
    let both = list_items_picking(&w, "s", Some(vec![0, 1, 2, 3]), false);
    let first = list_items_picking(&w, "s", Some(vec![0, 1, 2]), false);
    let second = list_items_picking(&w, "s", Some(vec![3]), false);
    assert!(first > 0 && second > 0);
    assert_eq!(first + second, both);
    assert_eq!(list_items_picking(&w, "s", Some(vec![]), false), 0);
    assert_eq!(list_items_picking(&w, "s", Some(vec![]), true), both);
}

/// Browser that removes the given group indices (sorted group order, as `tree::build`).
struct RemoveGroups(Vec<usize>);

impl Prompter for RemoveGroups {
    fn multi_select(&self, _title: &str, _options: &[String]) -> anyhow::Result<Vec<usize>> {
        unreachable!("browse is used instead")
    }
    fn confirm(&self, _q: &str) -> anyhow::Result<bool> {
        Ok(true)
    }
    fn browse(
        &self,
        _title: &str,
        _groups: &[skillz::tree::Node],
    ) -> anyhow::Result<Option<(Choice, Picked)>> {
        let picked = Picked {
            groups: self.0.clone(),
            items: vec![],
        };
        Ok(Some((Choice::Remove, picked)))
    }
}

#[test]
fn browser_remove_deletes_only_the_marked_group() {
    let w = world();
    let r = Recorder::default();
    let p = RemoveGroups(vec![3]); // groups: three "<agent> hooks", "mattpocock/skills"
    let d = Deps {
        home: w.home.clone(),
        claude_config: None,
        codex_home: None,
        cwd: w.project.clone(),
        adapters: agents::registry(),
        runner: &r,
        prompter: &p,
        interactive: true,
    };
    let cmd = Command::List {
        target: None,
        project: None,
        all: false,
        json: false,
        filter: Filter::default(),
        interactive: true,
    };
    run(cmd, &d, &mut Vec::new()).unwrap();
    assert!(!w.home.join(".agents/skills/tdd").exists());
    assert!(!w.home.join(".claude/skills/tdd").exists());
    let hooks = read_json(&w.home.join(".claude/settings.json"));
    assert!(!hooks["hooks"].as_object().unwrap().is_empty());
}

#[test]
fn prune_removes_duplicates_and_only_the_picked_unused_items() {
    let w = world();
    let cache = w.home.join(".claude/plugins/cache/mp/tools/1.0");
    write(
        &cache.join("skills/lint/SKILL.md"),
        "---\nname: lint\n---\n",
    );
    write(
        &w.home.join(".claude/plugins/installed_plugins.json"),
        &json!({"version": 2, "plugins": {"tools@mp": [
            {"scope": "user", "installPath": cache.to_string_lossy()}
        ]}})
        .to_string(),
    );
    write(
        &w.home.join(".claude/skills/lint/SKILL.md"),
        "---\nname: lint\n---\n",
    );
    let prune = |dry_run| Command::Prune {
        project: None,
        filter: Filter::default(),
        dry_run,
        force: false,
    };
    let (r, p) = (
        Recorder::default(),
        Scripted {
            pick: Some(vec![]),
            confirm: true,
        },
    );

    run(prune(true), &deps(&w, &r, &p), &mut Vec::new()).unwrap();
    assert!(w.home.join(".claude/skills/lint").exists());

    run(prune(false), &deps(&w, &r, &p), &mut Vec::new()).unwrap();
    assert!(!w.home.join(".claude/skills/lint").exists());
    assert!(w.home.join(".agents/skills/tdd").exists());
    assert!(cache.exists());
}

/// `main` swaps in a prompter that picks and confirms everything for `--force`; so does this one.
#[test]
fn prune_force_removes_duplicates_and_every_unused_item() {
    let w = world();
    let cache = w.home.join(".claude/plugins/cache/mp/tools/1.0");
    write(
        &cache.join("skills/lint/SKILL.md"),
        "---\nname: lint\n---\n",
    );
    write(
        &w.home.join(".claude/plugins/installed_plugins.json"),
        &json!({"version": 2, "plugins": {"tools@mp": [
            {"scope": "user", "installPath": cache.to_string_lossy()}
        ]}})
        .to_string(),
    );
    write(
        &w.home.join(".claude/skills/lint/SKILL.md"),
        "---\nname: lint\n---\n",
    );
    let cmd = Command::Prune {
        project: None,
        filter: Filter::default(),
        dry_run: false,
        force: true,
    };
    let (r, p) = (
        Recorder::default(),
        Scripted {
            pick: None,
            confirm: true,
        },
    );

    run(cmd, &deps(&w, &r, &p), &mut Vec::new()).unwrap();
    assert!(!w.home.join(".claude/skills/lint").exists());
    assert!(!w.home.join(".agents/skills/tdd").exists());
    assert!(!cache.exists());
}

fn global() -> Filter {
    Filter {
        global: true,
        ..Filter::default()
    }
}

fn reset(filter: Filter, keep: &[&str], dry_run: bool) -> Command {
    Command::Reset {
        project: None,
        filter,
        keep: keep.iter().map(|k| k.to_string()).collect(),
        dry_run,
        force: false,
    }
}

#[test]
fn reset_removes_skills_but_leaves_hooks_unless_asked() {
    let w = world();
    let (r, p) = (
        Recorder::default(),
        Scripted {
            pick: None,
            confirm: true,
        },
    );

    run(
        reset(global(), &[], true),
        &deps(&w, &r, &p),
        &mut Vec::new(),
    )
    .unwrap();
    assert!(w.home.join(".agents/skills/tdd").exists());

    run(
        reset(global(), &[], false),
        &deps(&w, &r, &p),
        &mut Vec::new(),
    )
    .unwrap();
    assert!(!w.home.join(".agents/skills/tdd").exists());
    let settings = read_json(&w.home.join(".claude/settings.json"));
    assert!(settings["hooks"]["PreToolUse"].is_array());

    let hooks = Filter {
        hooks: true,
        global: true,
        ..Filter::default()
    };
    run(reset(hooks, &[], false), &deps(&w, &r, &p), &mut Vec::new()).unwrap();
    let settings = read_json(&w.home.join(".claude/settings.json"));
    assert!(settings["hooks"].get("PreToolUse").is_none());
}

#[test]
fn reset_spares_what_keep_names_and_nothing_removed_when_declined() {
    let w = world();
    let declined = Scripted {
        pick: None,
        confirm: false,
    };
    let r = Recorder::default();
    run(
        reset(global(), &[], false),
        &deps(&w, &r, &declined),
        &mut Vec::new(),
    )
    .unwrap();
    assert!(w.home.join(".agents/skills/tdd").exists());

    let yes = Scripted {
        pick: None,
        confirm: true,
    };
    run(
        reset(global(), &["TDD"], false),
        &deps(&w, &r, &yes),
        &mut Vec::new(),
    )
    .unwrap();
    assert!(w.home.join(".agents/skills/tdd").exists());
}

fn use_cmd(profile: Option<&str>, dry_run: bool) -> Command {
    Command::Use {
        profile: profile.map(String::from),
        none: profile.is_none(),
        global: false,
        project: Some(PathBuf::from(".")),
        yes: true,
        dry_run,
    }
}

#[test]
fn use_profile_installs_once_then_swaps_and_unloads() {
    let w = world();
    let skills = |name: &str, skill: &str| {
        write(
            &w.home.join(format!(".config/skillz/profiles/{name}.toml")),
            &format!("[[entry]]\nkind = \"skill\"\nsource = \"o/r\"\nnames = [\"{skill}\"]\n"),
        )
    };
    skills("web", "tdd");
    skills("api", "grill");
    let (r, p) = (
        Recorder::default(),
        Scripted {
            pick: None,
            confirm: true,
        },
    );
    let d = deps(&w, &r, &p);
    let ledger = w.home.join(".config/skillz/applied.json");

    run(use_cmd(Some("web"), true), &d, &mut Vec::new()).unwrap();
    assert!(r.0.borrow().is_empty(), "dry run runs nothing");
    assert!(!ledger.exists());

    run(use_cmd(Some("web"), false), &d, &mut Vec::new()).unwrap();
    assert_eq!(r.0.borrow().len(), 1);
    assert!(fs::read_to_string(&ledger).unwrap().contains("web"));

    run(use_cmd(Some("web"), false), &d, &mut Vec::new()).unwrap();
    assert_eq!(r.0.borrow().len(), 1, "same profile again changes nothing");

    run(use_cmd(Some("api"), false), &d, &mut Vec::new()).unwrap();
    let calls = r.0.borrow().len();
    assert_eq!(calls, 2, "only the new profile's skill is installed");
    assert!(fs::read_to_string(&ledger).unwrap().contains("api"));

    run(use_cmd(None, false), &d, &mut Vec::new()).unwrap();
    assert!(!fs::read_to_string(&ledger).unwrap().contains("api"));

    assert!(run(use_cmd(Some("missing"), false), &d, &mut Vec::new()).is_err());
}
