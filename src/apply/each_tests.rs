//! What `run` tells its watcher while it runs item by item.

use std::path::{Path, PathBuf};

use super::*;
use crate::action::Action;
use crate::agents::registry;
use crate::env::Env;
use crate::model::{Agent, Installer, Kind, Scope};
use crate::profile::Spec;
use crate::review::{ItemEvent, Phase, Target};
use crate::runner::{CommandRunner, Output};
use crate::stale::{Disk, Probe};

/// Runs every program successfully except the one named.
struct Runner(&'static str);

impl CommandRunner for Runner {
    fn run(&self, program: &str, _args: &[String], _cwd: &Path) -> anyhow::Result<Output> {
        Ok(Output {
            success: program != self.0,
            stderr: "boom".into(),
        })
    }
}

fn entry(name: &str) -> Spec {
    Spec::Skill {
        source: "o/r".into(),
        names: vec![name.into()],
        agents: vec![],
    }
}

fn step(program: &str) -> Action {
    Action::Run {
        program: program.into(),
        args: vec![],
        cwd: PathBuf::from("."),
        fallback: false,
    }
}

fn installed_skill(name: &str) -> Installed {
    Installed {
        details: Default::default(),
        hooks: vec![],
        members: Vec::new(),
        id: format!("skill:{name}"),
        name: name.into(),
        kind: Kind::Skill,
        agents: vec![Agent::Claude],
        scope: Scope::Project,
        group: "o/r".into(),
        source: Some("o/r".into()),
        installer: Installer::Local,
        origin: Default::default(),
        children: vec![],
        removal: vec![],
    }
}

fn switch(installs: Vec<(Spec, Vec<Action>)>, removals: Vec<Installed>) -> Switch {
    Switch {
        to: "test".into(),
        present: vec![],
        removals,
        unsourced: vec![],
        installs,
        unsupported: vec![],
        updates: vec![],
    }
}

fn watched(switch: &Switch, runner: &dyn CommandRunner) -> (Report, Vec<ItemEvent>) {
    watched_with(switch, runner, &Disk)
}

fn watched_with(
    switch: &Switch,
    runner: &dyn CommandRunner,
    probe: &dyn Probe,
) -> (Report, Vec<ItemEvent>) {
    let (home, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let env = Env::new(home.path().into(), project.path().into());
    let adapters = registry();
    let applier = Applier {
        adapters: &adapters,
        env: &env,
        scope: Scope::Project,
        probe,
    };
    let mut events = Vec::new();
    let each = applier.run(
        switch,
        runner,
        &mut Watch {
            step: &mut |_| {},
            item: &mut |event| events.push(event),
        },
    );
    (each, events)
}

fn phases(events: &[ItemEvent]) -> Vec<(&Target, bool)> {
    events
        .iter()
        .map(|e| (&e.target, matches!(e.phase, Phase::Started)))
        .collect()
}

#[test]
fn each_install_starts_then_ends_before_the_next_begins() {
    let switch = switch(
        vec![
            (entry("a"), vec![step("good")]),
            (entry("b"), vec![step("good")]),
        ],
        vec![],
    );

    let (each, events) = watched(&switch, &Runner("none"));

    let (a, b) = (Target::Install(entry("a")), Target::Install(entry("b")));
    assert_eq!(
        phases(&events),
        [(&a, true), (&a, false), (&b, true), (&b, false)]
    );
    assert_eq!(each.installed, 2);
}

#[test]
fn a_failing_install_ends_failed_and_the_next_one_still_runs() {
    let switch = switch(
        vec![
            (entry("a"), vec![step("bad")]),
            (entry("b"), vec![step("good")]),
        ],
        vec![],
    );

    let (each, events) = watched(&switch, &Runner("bad"));

    let ended: Vec<bool> = events
        .iter()
        .filter(|e| e.phase != Phase::Started)
        .map(|e| matches!(e.phase, Phase::Succeeded))
        .collect();
    assert_eq!(ended, [false, true]);
    assert_eq!((each.installed, each.failures.len()), (1, 1));
}

#[test]
fn a_failed_shared_step_does_not_make_a_later_install_succeed() {
    let switch = switch(
        vec![
            (entry("a"), vec![step("bad")]),
            (entry("b"), vec![step("bad")]),
        ],
        vec![],
    );

    let (each, events) = watched(&switch, &Runner("bad"));

    assert_eq!((each.installed, each.failures.len()), (0, 2));
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e.phase, Phase::Failed(_)))
            .count(),
        2
    );
}

#[test]
fn a_removal_starts_and_ends_with_the_items_id() {
    let switch = switch(vec![], vec![installed_skill("old")]);

    let (_, events) = watched(&switch, &Runner("none"));

    let target = Target::Remove("skill:old".into());
    assert_eq!(phases(&events), [(&target, true), (&target, false)]);
    assert_eq!(events[1].phase, Phase::Succeeded);
}

/// Records the programs it is asked to run.
struct Recording(std::cell::RefCell<Vec<String>>);

impl CommandRunner for Recording {
    fn run(&self, program: &str, _args: &[String], _cwd: &Path) -> anyhow::Result<Output> {
        self.0.borrow_mut().push(program.into());
        Ok(Output {
            success: true,
            stderr: String::new(),
        })
    }
}

#[test]
fn an_entry_whose_local_source_is_gone_fails_before_running_anything_and_the_rest_still_run() {
    let gone = Spec::Skill {
        source: "/old/skills".into(),
        names: vec!["mine".into()],
        agents: vec![],
    };
    let switch = switch(
        vec![(gone, vec![step("npx")]), (entry("b"), vec![step("good")])],
        vec![],
    );
    let runner = Recording(Default::default());

    let (each, _) = watched_with(&switch, &runner, &crate::stale::tests::Existing::of(&[]));

    // Assert the missing source never reached the runner, and only the other entry installed.
    assert_eq!(*runner.0.borrow(), ["good"]);
    assert_eq!((each.installed, each.failures.len()), (1, 1));
}
