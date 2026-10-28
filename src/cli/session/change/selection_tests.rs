use std::path::Path;

use super::tests::{project_with_skill, run_change, session, skill_entry, skill_id};
use super::*;
use crate::app::install::test_support::ScriptedPrompter;
use skillz_harness::Spec;
use skillz_harness::agents::registry;
use skillz_harness::runner::{CommandRunner, Output};

/// Fails every command that mentions `word`, and notes the arguments of all of them.
struct FailsOn {
    word: String,
    calls: std::cell::RefCell<Vec<Vec<String>>>,
}

impl FailsOn {
    fn new(word: &str) -> Self {
        FailsOn {
            word: word.into(),
            calls: Default::default(),
        }
    }
}

impl CommandRunner for FailsOn {
    fn run(&self, _program: &str, args: &[String], _cwd: &Path) -> anyhow::Result<Output> {
        self.calls.borrow_mut().push(args.to_vec());
        Ok(Output {
            success: !args.contains(&self.word),
            stderr: "boom".into(),
        })
    }
}

fn selection<'a>(install: &'a [Spec], remove: &'a [String]) -> Selection<'a> {
    Selection {
        install,
        remove,
        only: None,
        packages: false,
        npx: None,
    }
}

#[test]
fn removing_an_approved_item_deletes_it_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    project_with_skill(dir.path(), "tdd");
    let ids = [skill_id(dir.path())];

    let (result, _) = run_change(dir.path(), true, &Change::Selection(&selection(&[], &ids)));

    assert!(matches!(
        result.unwrap(),
        Some(Notice::Changed { removed: 1, .. })
    ));
    assert!(!dir.path().join(".agents/skills/tdd").exists());
}

#[test]
fn a_declined_plan_changes_nothing_and_reports_nothing() {
    let dir = tempfile::tempdir().unwrap();
    project_with_skill(dir.path(), "tdd");
    let ids = [skill_id(dir.path())];

    let (result, _) = run_change(dir.path(), false, &Change::Selection(&selection(&[], &ids)));

    assert!(result.unwrap().is_none());
    assert!(dir.path().join(".agents/skills/tdd").exists());
}

#[test]
fn removing_something_already_gone_needs_no_plan() {
    let dir = tempfile::tempdir().unwrap();
    let ids = ["skill:gone".to_string()];

    let (result, _) = run_change(dir.path(), true, &Change::Selection(&selection(&[], &ids)));

    assert!(matches!(result.unwrap(), Some(Notice::UpToDate)));
}

#[test]
fn installs_and_removals_run_as_one_change() {
    let dir = tempfile::tempdir().unwrap();
    project_with_skill(dir.path(), "tdd");
    let ids = [skill_id(dir.path())];
    let entries = [skill_entry("grill"), skill_entry("rust")];

    let (result, runner) = run_change(
        dir.path(),
        true,
        &Change::Selection(&selection(&entries, &ids)),
    );

    let adds = runner
        .calls()
        .iter()
        .filter(|(_, args, _)| args.contains(&"add".to_string()))
        .count();
    assert!(matches!(
        result.unwrap(),
        Some(Notice::Changed {
            installed: 2,
            removed: 1
        })
    ));
    assert_eq!(adds, 2);
    assert!(!dir.path().join(".agents/skills/tdd").exists());
}

#[test]
fn installing_what_is_already_there_needs_no_plan() {
    let dir = tempfile::tempdir().unwrap();
    project_with_skill(dir.path(), "tdd");
    let entries = [skill_entry("tdd")];

    let (result, runner) = run_change(
        dir.path(),
        true,
        &Change::Selection(&selection(&entries, &[])),
    );

    assert!(matches!(result.unwrap(), Some(Notice::UpToDate)));
    assert!(runner.calls().is_empty());
}

#[test]
fn one_failing_install_is_reported_and_the_others_still_run() {
    let dir = tempfile::tempdir().unwrap();
    let entries = [skill_entry("bad"), skill_entry("good")];
    let runner = FailsOn::new("bad");
    let prompter = ScriptedPrompter::new(&[], true);
    let deps = Deps::for_test(dir.path(), registry(), (&runner, &prompter));

    let picked = selection(&entries, &[]);
    let result = change(
        &session(&deps, dir.path()),
        Scope::Project,
        &Change::Selection(&picked),
    );

    let tried_good = runner
        .calls
        .borrow()
        .iter()
        .any(|a| a.contains(&"good".to_string()));
    assert!(matches!(result.unwrap(), Some(Notice::Failed(_))));
    assert!(tried_good);
}

#[test]
fn an_unsupported_install_is_skipped_without_counting_as_installed() {
    let dir = tempfile::tempdir().unwrap();
    let entries = [Spec::Mcp {
        name: "srv".into(),
        command: Some("npx".into()),
        args: vec![],
        url: None,
        transport: None,
        agents: vec![skillz_harness::model::Agent::Codex],
    }];
    let runner = crate::app::install::test_support::FakeRunner::new(|_| {});
    let prompter = super::tests::Outcomes::default();
    let deps = Deps::for_test(dir.path(), registry(), (&runner, &prompter));

    change(
        &session(&deps, dir.path()),
        Scope::Project,
        &Change::Selection(&selection(&entries, &[])),
    )
    .unwrap();

    let values = prompter.values.borrow();
    assert_eq!((values[0].installed, values[0].skipped), (0, 1));
    assert!(runner.calls().is_empty());
}

#[test]
fn a_failed_removal_is_not_counted_as_removed() {
    let dir = tempfile::tempdir().unwrap();
    project_with_skill(dir.path(), "tdd");
    let ids = [skill_id(dir.path())];
    let runner = crate::app::install::test_support::FakeRunner::new(|_| {}).failing("npx");
    let prompter = super::tests::Outcomes::default();
    let deps = Deps::for_test(dir.path(), registry(), (&runner, &prompter));

    change(
        &session(&deps, dir.path()),
        Scope::Project,
        &Change::Selection(&selection(&[], &ids)),
    )
    .unwrap();

    let values = prompter.values.borrow();
    assert_eq!(values[0].removed, 0);
    assert!(!values[0].is_clean());
    assert!(dir.path().join(".agents/skills/tdd").exists());
}

/// Unchecks the removal and one install in the first plan it is shown, then approves the second.
struct UnchecksFirst {
    install: String,
    plans: std::cell::Cell<usize>,
}

impl crate::app::prompt::Prompter for UnchecksFirst {
    fn multi_select(&self, _title: &str, _options: &[String]) -> anyhow::Result<Vec<usize>> {
        Ok(vec![])
    }

    fn confirm(&self, _question: &str) -> anyhow::Result<bool> {
        Ok(true)
    }

    fn plan(&self, view: &crate::app::review::PlanView) -> anyhow::Result<Verdict> {
        self.plans.set(self.plans.get() + 1);
        if self.plans.get() > 1 {
            return Ok(Verdict::Apply);
        }
        let mut targets = view.targets.clone().expect("a selection offers its items");
        for pick in &mut targets.items {
            if pick.group == crate::app::review::Group::Remove || pick.id == self.install {
                pick.on = false;
            }
        }
        Ok(Verdict::Retarget(targets))
    }
}

#[test]
fn an_item_the_user_unchecks_is_neither_installed_nor_removed() {
    let dir = tempfile::tempdir().unwrap();
    project_with_skill(dir.path(), "tdd");
    let ids = [skill_id(dir.path())];
    let entries = [skill_entry("grill"), skill_entry("rust")];
    let runner = crate::app::install::test_support::FakeRunner::new(|_| {});
    let prompter = UnchecksFirst {
        install: entries[0].to_string(),
        plans: Default::default(),
    };
    let deps = Deps::for_test(dir.path(), registry(), (&runner, &prompter));

    let result = change(
        &session(&deps, dir.path()),
        Scope::Project,
        &Change::Selection(&selection(&entries, &ids)),
    );

    let adds: Vec<_> = runner
        .calls()
        .into_iter()
        .filter(|(_, args, _)| args.contains(&"add".to_string()))
        .collect();
    assert!(matches!(
        result.unwrap(),
        Some(Notice::Changed {
            installed: 1,
            removed: 0
        })
    ));
    assert_eq!(adds.len(), 1);
    assert!(dir.path().join(".agents/skills/tdd").exists());
}

/// Unchecks every removal in the first plan, notes whether each plan shown is destructive, then approves.
struct UnchecksRemovals {
    destructive: std::cell::RefCell<Vec<bool>>,
}

impl crate::app::prompt::Prompter for UnchecksRemovals {
    fn multi_select(&self, _title: &str, _options: &[String]) -> anyhow::Result<Vec<usize>> {
        Ok(vec![])
    }

    fn confirm(&self, _question: &str) -> anyhow::Result<bool> {
        Ok(true)
    }

    fn plan(&self, view: &crate::app::review::PlanView) -> anyhow::Result<Verdict> {
        self.destructive.borrow_mut().push(view.destructive());
        if self.destructive.borrow().len() > 1 {
            return Ok(Verdict::Apply);
        }
        let mut targets = view.targets.clone().expect("a selection offers its items");
        for pick in &mut targets.items {
            pick.on = pick.group != crate::app::review::Group::Remove;
        }
        Ok(Verdict::Retarget(targets))
    }
}

#[test]
fn a_plan_whose_removals_are_all_unchecked_is_no_longer_destructive() {
    let dir = tempfile::tempdir().unwrap();
    project_with_skill(dir.path(), "tdd");
    let ids = [skill_id(dir.path())];
    let entries = [skill_entry("grill")];
    let runner = crate::app::install::test_support::FakeRunner::new(|_| {});
    let prompter = UnchecksRemovals {
        destructive: Default::default(),
    };
    let deps = Deps::for_test(dir.path(), registry(), (&runner, &prompter));

    change(
        &session(&deps, dir.path()),
        Scope::Project,
        &Change::Selection(&selection(&entries, &ids)),
    )
    .unwrap();

    assert_eq!(*prompter.destructive.borrow(), [true, false]);
}

fn package<'a>(install: &'a [Spec]) -> Selection<'a> {
    Selection {
        packages: true,
        ..selection(install, &[])
    }
}

fn plugin_entry(source: &str) -> Spec {
    Spec::Plugin {
        source: source.into(),
        marketplace: "mp".into(),
        name: "p".into(),
        agents: vec![],
    }
}

fn npx_calls(runner: &crate::app::install::test_support::FakeRunner) -> Vec<Vec<String>> {
    runner
        .calls()
        .into_iter()
        .filter(|(program, _, _)| program == "npx")
        .map(|(_, args, _)| args)
        .collect()
}

#[test]
fn a_package_of_skills_installs_with_one_npx_run_for_the_whole_source() {
    let dir = tempfile::tempdir().unwrap();
    let entries = [
        skill_entry("grill"),
        skill_entry("rust"),
        skill_entry("tdd"),
    ];

    let (result, runner) = run_change(dir.path(), true, &Change::Selection(&package(&entries)));

    let npx = npx_calls(&runner);
    assert!(matches!(
        result.unwrap(),
        Some(Notice::Changed { installed: 1, .. })
    ));
    assert_eq!(npx.len(), 1);
    assert!(npx[0].windows(2).any(|pair| pair == ["--skill", "*"]));
}

#[test]
fn a_package_with_a_plugin_installs_the_plugin_and_leaves_its_skills_to_it() {
    let dir = tempfile::tempdir().unwrap();
    let source = match skill_entry("grill") {
        Spec::Skill { source, .. } => source,
        other => panic!("{other:?}"),
    };
    let entries = [skill_entry("grill"), plugin_entry(&source)];

    let (_, runner) = run_change(dir.path(), true, &Change::Selection(&package(&entries)));

    let programs: Vec<String> = runner.calls().into_iter().map(|c| c.0).collect();
    assert!(npx_calls(&runner).is_empty());
    assert!(programs.contains(&"claude".to_string()));
}

/// Checks `npx` in the first plan it is shown, then approves.
struct ChecksNpx {
    seen: std::cell::RefCell<Vec<Option<bool>>>,
}

impl crate::app::prompt::Prompter for ChecksNpx {
    fn multi_select(&self, _title: &str, _options: &[String]) -> anyhow::Result<Vec<usize>> {
        Ok(vec![])
    }

    fn confirm(&self, _question: &str) -> anyhow::Result<bool> {
        Ok(true)
    }

    fn plan(&self, view: &crate::app::review::PlanView) -> anyhow::Result<Verdict> {
        let targets = view
            .targets
            .clone()
            .expect("an install offers its installers");
        self.seen.borrow_mut().push(targets.npx);
        if self.seen.borrow().len() > 1 {
            return Ok(Verdict::Apply);
        }
        Ok(Verdict::Retarget(Targets {
            npx: Some(true),
            ..targets
        }))
    }
}

#[test]
fn checking_npx_runs_it_next_to_the_agents_plugin_installers() {
    let dir = tempfile::tempdir().unwrap();
    let source = match skill_entry("grill") {
        Spec::Skill { source, .. } => source,
        other => panic!("{other:?}"),
    };
    let entries = [skill_entry("grill"), plugin_entry(&source)];
    let runner = crate::app::install::test_support::FakeRunner::new(|_| {});
    let prompter = ChecksNpx {
        seen: Default::default(),
    };
    let deps = Deps::for_test(dir.path(), registry(), (&runner, &prompter));

    change(
        &session(&deps, dir.path()),
        Scope::Project,
        &Change::Selection(&package(&entries)),
    )
    .unwrap();

    let programs: Vec<String> = runner.calls().into_iter().map(|c| c.0).collect();
    assert_eq!(*prompter.seen.borrow(), [Some(false), Some(true)]);
    assert_eq!(npx_calls(&runner).len(), 1);
    assert!(programs.contains(&"claude".to_string()));
}
