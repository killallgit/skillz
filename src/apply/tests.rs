//! Applying profiles against a fake `npx skills` that installs into a temp project.

use std::cell::RefCell;
use std::fs;
use std::path::Path;

use super::*;
use crate::agents::registry;
use crate::model::Agent;
use crate::profile::{Entry, Profile};
use crate::runner::{CommandRunner, Output};
use crate::stale::Disk;

/// Installs the skills named by `--skill` into the project like `npx skills add` would: the
/// files, the link for Claude and the lock entry that records the source.
struct NpxFake {
    project: std::path::PathBuf,
    calls: RefCell<Vec<Vec<String>>>,
    fail: bool,
}

impl NpxFake {
    fn new(project: &Path) -> Self {
        NpxFake {
            project: project.into(),
            calls: RefCell::default(),
            fail: false,
        }
    }

    fn failing(project: &Path) -> Self {
        NpxFake {
            fail: true,
            ..Self::new(project)
        }
    }

    fn count(&self) -> usize {
        self.calls.borrow().len()
    }
}

impl CommandRunner for NpxFake {
    fn run(&self, _program: &str, args: &[String], _cwd: &Path) -> anyhow::Result<Output> {
        self.calls.borrow_mut().push(args.to_vec());
        if !self.fail && args.iter().any(|a| a == "add") {
            for pair in args.windows(2).filter(|w| w[0] == "--skill") {
                install_skill(&self.project, &pair[1]);
            }
        }
        Ok(Output {
            success: !self.fail,
            stderr: "boom".into(),
        })
    }
}

fn install_skill(root: &Path, name: &str) {
    write_skill(root, name);
    let links = root.join(".claude/skills");
    fs::create_dir_all(&links).unwrap();
    std::os::unix::fs::symlink(root.join(".agents/skills").join(name), links.join(name)).unwrap();
    let lock = root.join("skills-lock.json");
    let mut skills = fs::read_to_string(&lock)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .map_or_else(|| serde_json::json!({}), |v| v["skills"].clone());
    skills[name] = serde_json::json!({"source": "o/r"});
    fs::write(lock, serde_json::json!({"skills": skills}).to_string()).unwrap();
}

fn write_skill(root: &Path, name: &str) {
    let dir = root.join(".agents/skills").join(name);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: d\n---\n"),
    )
    .unwrap();
}

struct Fx {
    _dirs: (tempfile::TempDir, tempfile::TempDir),
    env: Env,
    adapters: Vec<Box<dyn AgentAdapter>>,
}

impl Fx {
    fn new() -> Self {
        let (home, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        Fx {
            env: Env::new(home.path().into(), project.path().into()),
            adapters: registry(),
            _dirs: (home, project),
        }
    }

    fn applier(&self) -> Applier<'_> {
        Applier {
            adapters: &self.adapters,
            env: &self.env,
            scope: Scope::Project,
            probe: &Disk,
        }
    }

    fn npx(&self) -> NpxFake {
        NpxFake::new(&self.env.project)
    }

    fn plan(&self, target: &Profile, exact: bool) -> Switch {
        self.applier().plan(target, exact).unwrap()
    }

    fn run(&self, switch: &Switch, runner: &dyn CommandRunner) -> Report {
        self.applier().run(
            switch,
            runner,
            &mut Watch {
                step: &mut |_| {},
                item: &mut |_| {},
            },
        )
    }

    fn apply(&self, target: &Profile, exact: bool, runner: &dyn CommandRunner) -> Report {
        {
            let report = self.run(&self.plan(target, exact), runner);
            assert!(report.failures.is_empty());
            report
        }
    }

    fn has_skill(&self, name: &str) -> bool {
        self.env.project.join(".agents/skills").join(name).exists()
    }

    fn matches(&self, target: &Profile) -> bool {
        let installed = self.applier().installed().unwrap();
        compare(&target.resolved(), &installed).matches()
    }
}

/// A Claude profile with one skill entry per name.
fn profile(name: &str, skills: &[&str]) -> Profile {
    let mut p = Profile::new(name);
    p.agents = vec![Agent::Claude];
    p.entries = skills
        .iter()
        .map(|s| Entry::Skill {
            source: "o/r".into(),
            names: vec![s.to_string()],
            agents: vec![],
        })
        .collect();
    p
}

#[test]
fn applying_installs_what_is_missing_and_then_the_project_matches() {
    let fx = Fx::new();
    let web = profile("web", &["tdd"]);

    fx.apply(&web, false, &fx.npx());

    assert!(fx.has_skill("tdd"));
    assert!(fx.matches(&web));
}

#[test]
fn applying_never_removes_what_the_profile_does_not_name() {
    let fx = Fx::new();
    let runner = fx.npx();
    install_skill(&fx.env.project, "other");
    write_skill(&fx.env.project, "mine");

    fx.apply(&profile("web", &["tdd"]), false, &runner);

    assert!(fx.has_skill("other") && fx.has_skill("mine") && fx.has_skill("tdd"));
}

#[test]
fn applying_what_is_all_there_plans_and_runs_nothing() {
    let fx = Fx::new();
    let runner = fx.npx();
    let web = profile("web", &["tdd"]);
    fx.apply(&web, false, &runner);

    let again = fx.plan(&web, false);

    assert!(again.is_empty());
    assert_eq!(runner.count(), 1);
}

#[test]
fn only_the_missing_skills_are_installed() {
    let fx = Fx::new();
    let runner = fx.npx();
    install_skill(&fx.env.project, "tdd");

    let switch = fx.plan(&profile("web", &["tdd", "new"]), false);
    fx.run(&switch, &runner);

    assert_eq!(switch.present.len(), 1);
    assert_eq!(switch.installs.len(), 1);
    assert_eq!(runner.count(), 1);
}

#[test]
fn duplicate_entries_install_once() {
    let fx = Fx::new();
    let runner = fx.npx();

    fx.apply(&profile("a", &["tdd", "tdd"]), false, &runner);

    assert_eq!(runner.count(), 1);
}

#[test]
fn a_profile_for_another_agent_is_not_matched_by_a_claude_install() {
    let fx = Fx::new();
    install_skill(&fx.env.project, "tdd");
    let mut cursor = profile("c", &["tdd"]);
    cursor.agents = vec![Agent::Cursor];

    assert!(!fx.matches(&cursor));
}

#[test]
fn exact_removes_what_the_profile_does_not_want_and_keeps_what_it_does() {
    let fx = Fx::new();
    let runner = fx.npx();
    install_skill(&fx.env.project, "tdd");
    install_skill(&fx.env.project, "old");

    let switch = fx.plan(&profile("web", &["tdd"]), true);
    let done = fx.run(&switch, &runner);

    assert_eq!(switch.removals.len(), 1);
    assert!(done.removed.unwrap().still_installed.is_empty());
    assert!(fx.has_skill("tdd") && !fx.has_skill("old"));
}

#[test]
fn exact_flags_an_item_no_profile_could_install_again() {
    let fx = Fx::new();
    write_skill(&fx.env.project, "mine");
    fs::write(
        fx.env.project.join(".agents/skills/mine/SKILL.md"),
        "---\nname: mine\n---\n",
    )
    .unwrap();
    fs::create_dir_all(fx.env.project.join(".claude/skills")).unwrap();
    std::os::unix::fs::symlink(
        fx.env.project.join(".agents/skills/mine"),
        fx.env.project.join(".claude/skills/mine"),
    )
    .unwrap();

    let switch = fx.plan(&profile("web", &[]), true);

    assert_eq!(switch.removals.len(), 1);
    assert!(switch.is_unsourced(&switch.removals[0]));
}

#[test]
fn exact_has_nothing_to_do_when_nothing_is_extra() {
    let fx = Fx::new();
    let runner = fx.npx();
    let web = profile("web", &["tdd"]);
    fx.apply(&web, false, &runner);

    let exact = fx.plan(&web, true);

    assert!(exact.is_empty());
}

#[test]
fn a_failed_install_is_an_error() {
    let fx = Fx::new();
    let runner = NpxFake::failing(&fx.env.project);
    let switch = fx.plan(&profile("a", &["tdd"]), false);

    let result = fx.run(&switch, &runner);

    assert_eq!(result.failures.len(), 1);
}

#[test]
fn a_hook_lands_in_the_settings_and_an_exact_empty_profile_takes_it_out_again() {
    let fx = Fx::new();
    let settings = fx.env.project.join(".claude/settings.local.json");
    let mut p = Profile::new("notify");
    p.entries.push(Entry::Hook {
        event: "Stop".into(),
        matcher: None,
        command: "say done".into(),
        agents: vec![Agent::Claude],
    });
    let runner = fx.npx();

    fx.apply(&p, false, &runner);
    let loaded = fs::read_to_string(&settings).unwrap();
    fx.apply(&Profile::new("none"), true, &runner);
    let unloaded = fs::read_to_string(&settings).unwrap();

    assert!(loaded.contains("say done"));
    assert!(!unloaded.contains("say done"));
}

#[test]
fn an_unreadable_inventory_stops_planning_instead_of_guessing() {
    let fx = Fx::new();
    let settings = fx.env.project.join(".claude/settings.local.json");
    fs::create_dir_all(settings.parent().unwrap()).unwrap();
    fs::write(&settings, "{nope").unwrap();

    let planned = fx.applier().plan(&profile("web", &["tdd"]), false);

    assert!(planned.is_err());
}

#[test]
fn a_relative_local_subagent_source_matches_its_canonical_install() {
    let fx = Fx::new();
    let source = fx.env.project.join("definitions");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("reviewer.md"), "---\nname: reviewer\n---\nbody").unwrap();
    let mut profile = Profile::new("review");
    profile.entries.push(Entry::Subagent {
        source: "./definitions".into(),
        names: vec!["reviewer".into()],
        agents: vec![Agent::Claude],
    });
    fx.apply(&profile, false, &fx.npx());

    let again = fx.plan(&profile, true);

    assert!(again.is_empty());
    assert_eq!(again.present.len(), 1);
}
