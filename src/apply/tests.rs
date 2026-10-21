//! Switching profiles against a fake `npx skills` that installs into a temp project.

use std::cell::RefCell;
use std::fs;
use std::path::Path;

use super::*;
use crate::agents::registry;
use crate::model::Agent;
use crate::profile::{Entry, Profile};
use crate::runner::{CommandRunner, Output};

/// Installs the skills named by `--skill` into the project's npx store when asked to
/// `skills add`, like the real tool would; every command fails when `fail` is set.
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
                write_skill(&self.project, &pair[1]);
            }
        }
        Ok(Output {
            success: !self.fail,
            stderr: "boom".into(),
        })
    }
}

/// Lets the skill land, then fails the plugin command.
struct FailsOnClaude(NpxFake);

impl CommandRunner for FailsOnClaude {
    fn run(&self, program: &str, args: &[String], cwd: &Path) -> anyhow::Result<Output> {
        if program == "claude" {
            return Ok(Output {
                success: false,
                stderr: "no".into(),
            });
        }
        self.0.run(program, args, cwd)
    }
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
    ledger: Ledger,
}

impl Fx {
    fn new() -> Self {
        let (home, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let env = Env::new(home.path().into(), project.path().into());
        Fx {
            ledger: Ledger::in_env(&env),
            adapters: registry(),
            env,
            _dirs: (home, project),
        }
    }

    fn applier(&self) -> Applier<'_> {
        Applier {
            adapters: &self.adapters,
            env: &self.env,
            scope: Scope::Project,
            ledger: &self.ledger,
        }
    }

    fn npx(&self) -> NpxFake {
        NpxFake::new(&self.env.project)
    }

    fn plan(&self, target: Option<&Profile>) -> Switch {
        self.applier().plan(target).unwrap()
    }

    fn run(&self, switch: &Switch, runner: &dyn CommandRunner) -> anyhow::Result<Done> {
        self.applier().run(switch, runner, &mut |_| {})
    }

    fn switch_to(&self, target: Option<&Profile>, runner: &dyn CommandRunner) -> Done {
        self.run(&self.plan(target), runner).unwrap()
    }

    fn record(&self) -> Option<Record> {
        self.ledger
            .get(&Ledger::key(&self.env, Scope::Project))
            .unwrap()
    }

    fn owned_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .record()
            .into_iter()
            .flat_map(|r| r.entries)
            .flat_map(|l| l.items)
            .map(|o| o.name)
            .collect();
        names.sort();
        names
    }

    fn has_skill(&self, name: &str) -> bool {
        self.env.project.join(".agents/skills").join(name).exists()
    }
}

/// A profile with one skill entry per name.
fn profile(name: &str, skills: &[&str]) -> Profile {
    let mut p = Profile::new(name);
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

fn plugin_entry() -> Entry {
    Entry::Plugin {
        source: "o/mp".into(),
        marketplace: "mp".into(),
        name: "pp".into(),
        agents: vec![Agent::Claude],
    }
}

#[test]
fn loading_a_profile_records_what_appeared_and_unloading_removes_exactly_that() {
    let fx = Fx::new();
    let by_hand = fx.env.project.join(".claude/skills/mine");
    fs::create_dir_all(&by_hand).unwrap();
    fs::write(
        by_hand.join("SKILL.md"),
        "---\nname: mine\ndescription: d\n---\n",
    )
    .unwrap();
    let runner = fx.npx();

    let loaded = fx.switch_to(Some(&profile("web", &["tdd"])), &runner);
    assert_eq!(loaded.added.len(), 1);
    assert_eq!(fx.record().unwrap().profile, "web");

    let unloaded = fx.switch_to(None, &runner);

    assert!(unloaded.removed.unwrap().still_installed.is_empty());
    assert!(!fx.has_skill("tdd"));
    assert!(by_hand.exists(), "hand-installed skill survives");
    assert!(fx.record().is_none());
}

#[test]
fn items_that_were_already_installed_are_not_owned() {
    let fx = Fx::new();
    write_skill(&fx.env.project, "tdd");
    let runner = fx.npx();

    let loaded = fx.switch_to(Some(&profile("web", &["tdd"])), &runner);
    fx.switch_to(None, &runner);

    assert!(loaded.added.is_empty());
    assert!(fx.has_skill("tdd"));
}

#[test]
fn loading_the_same_profile_again_changes_nothing() {
    let fx = Fx::new();
    let runner = fx.npx();
    let web = profile("web", &["tdd"]);
    fx.switch_to(Some(&web), &runner);

    let again = fx.plan(Some(&web));

    assert!(again.is_empty());
    assert!(again.steps().is_empty());
    assert_eq!(runner.count(), 1);
}

#[test]
fn swapping_touches_only_the_difference() {
    let fx = Fx::new();
    let runner = fx.npx();
    fx.switch_to(Some(&profile("a", &["tdd", "old"])), &runner);

    let switch = fx.plan(Some(&profile("b", &["tdd", "new"])));
    fx.run(&switch, &runner).unwrap();

    assert_eq!(
        (switch.keep.len(), switch.drop.len(), switch.installs.len()),
        (1, 1, 1)
    );
    assert_eq!(runner.count(), 3, "only the new skill was installed");
    assert!(fx.has_skill("tdd") && fx.has_skill("new") && !fx.has_skill("old"));
    assert_eq!(fx.owned_names(), ["new", "tdd"]);
    assert_eq!(fx.record().unwrap().profile, "b");
}

#[test]
fn swapping_names_where_it_came_from_and_where_it_goes() {
    let fx = Fx::new();
    fx.switch_to(Some(&profile("a", &["tdd"])), &fx.npx());

    let switch = fx.plan(Some(&profile("b", &["tdd"])));

    assert_eq!(switch.from.as_deref(), Some("a"));
    assert_eq!(switch.to.as_deref(), Some("b"));
}

#[test]
fn duplicate_entries_install_once() {
    let fx = Fx::new();
    let runner = fx.npx();

    fx.switch_to(Some(&profile("a", &["tdd", "tdd"])), &runner);

    assert_eq!(runner.count(), 1);
}

#[test]
fn a_profile_whose_agents_differ_is_a_different_entry() {
    let fx = Fx::new();
    fx.switch_to(Some(&profile("a", &["tdd"])), &fx.npx());
    let mut b = profile("b", &["tdd"]);
    b.agents = vec![Agent::Codex];

    let switch = fx.plan(Some(&b));

    assert_eq!(
        (switch.keep.len(), switch.drop.len(), switch.installs.len()),
        (0, 1, 1)
    );
}

#[test]
fn a_failed_install_leaves_no_record_when_nothing_landed() {
    let fx = Fx::new();
    let runner = NpxFake::failing(&fx.env.project);
    let switch = fx.plan(Some(&profile("a", &["tdd"])));

    let result = fx.run(&switch, &runner);

    assert!(result.is_err());
    assert!(fx.record().is_none());
}

#[test]
fn unloading_with_nothing_loaded_does_nothing() {
    let fx = Fx::new();
    let runner = fx.npx();
    let switch = fx.plan(None);

    fx.run(&switch, &runner).unwrap();

    assert!(switch.is_empty());
    assert_eq!(runner.count(), 0);
    assert!(fx.record().is_none());
}

#[test]
fn a_partial_install_is_recorded_so_it_can_be_unloaded() {
    let fx = Fx::new();
    let mut p = profile("web", &["tdd"]);
    p.entries.push(plugin_entry());
    let runner = FailsOnClaude(fx.npx());

    let result = fx.run(&fx.plan(Some(&p)), &runner);
    let recorded = fx.owned_names();
    fx.switch_to(None, &runner);

    assert!(result.is_err());
    assert_eq!(recorded, ["tdd"]);
    assert!(!fx.has_skill("tdd"));
}

#[test]
fn a_corrupt_ledger_stops_planning() {
    let fx = Fx::new();
    fs::create_dir_all(fx.env.skillz_dir()).unwrap();
    fs::write(fx.env.skillz_dir().join("applied.json"), "{nope").unwrap();

    assert!(fx.applier().plan(None).is_err());
}
