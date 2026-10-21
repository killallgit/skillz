//! What `run_each` tells its watcher while it runs item by item.

use std::path::{Path, PathBuf};

use super::*;
use crate::action::Action;
use crate::agents::registry;
use crate::env::Env;
use crate::model::{Agent, Installer, Kind, Scope};
use crate::profile::Entry;
use crate::review::{ItemEvent, Phase, Target};
use crate::runner::{CommandRunner, Output};

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

fn entry(name: &str) -> Entry {
    Entry::Skill {
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

fn installed_skill(name: &str) -> Augmentation {
    Augmentation {
        hooks: vec![],
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

fn switch(installs: Vec<(Entry, Vec<Action>)>, removals: Vec<Augmentation>) -> Switch {
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

fn watched(switch: &Switch, runner: &dyn CommandRunner) -> (Each, Vec<ItemEvent>) {
    let (home, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let env = Env::new(home.path().into(), project.path().into());
    let adapters = registry();
    let applier = Applier {
        adapters: &adapters,
        env: &env,
        scope: Scope::Project,
    };
    let mut events = Vec::new();
    let each = applier.run_each(
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
fn a_removal_starts_and_ends_with_the_items_id() {
    let switch = switch(vec![], vec![installed_skill("old")]);

    let (_, events) = watched(&switch, &Runner("none"));

    let target = Target::Remove("skill:old".into());
    assert_eq!(phases(&events), [(&target, true), (&target, false)]);
    assert_eq!(events[1].phase, Phase::Succeeded);
}
