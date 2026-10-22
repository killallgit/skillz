use std::path::Path;

use super::tests::{project_with_skill, run_change, session, skill_entry, skill_id};
use super::*;
use crate::agents::registry;
use crate::install::test_support::ScriptedPrompter;
use crate::profile::Entry;
use crate::runner::{CommandRunner, Output};

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

fn selection<'a>(install: &'a [Entry], remove: &'a [String]) -> Selection<'a> {
    Selection {
        install,
        remove,
        only: None,
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
    let entries = [Entry::Plugin {
        source: "o/r".into(),
        marketplace: "mp".into(),
        name: "p".into(),
        agents: vec![crate::model::Agent::Cursor],
    }];
    let runner = crate::install::test_support::FakeRunner::new(|_| {});
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
    let runner = crate::install::test_support::FakeRunner::new(|_| {}).failing("npx");
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
