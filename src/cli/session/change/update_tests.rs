use std::fs;
use std::path::Path;

use super::tests::{project_with_skill, run_change, session, skill_id};
use super::*;
use crate::app::install::test_support::{FakeNpx, FakeRunner, ScriptedPrompter};
use crate::app::workspace::Act;
use skillz_harness::agents::registry;

fn locked_at(dir: &Path, hash: &str) {
    project_with_skill(dir, "tdd");
    let lock = serde_json::json!({"skills": {"tdd": {"source": "o/r", "computedHash": hash}}});
    fs::write(dir.join("skills-lock.json"), lock.to_string()).unwrap();
}

#[test]
fn updating_a_skill_runs_the_update_of_the_tool_that_installed_it() {
    let dir = tempfile::tempdir().unwrap();
    locked_at(dir.path(), "old");
    let ids = [skill_id(dir.path())];

    let (result, runner) = run_change(dir.path(), true, &Change::Update(&ids));

    let calls = runner.calls();
    assert!(matches!(result.unwrap(), Some(Notice::ItemsUpdated(1))));
    assert_eq!(
        (calls[0].0.as_str(), calls[0].1[2].as_str()),
        ("npx", "update")
    );
}

#[test]
fn a_declined_update_runs_nothing() {
    let dir = tempfile::tempdir().unwrap();
    locked_at(dir.path(), "old");
    let ids = [skill_id(dir.path())];

    let (result, runner) = run_change(dir.path(), false, &Change::Update(&ids));

    assert!(result.unwrap().is_none());
    assert!(runner.calls().is_empty());
}

#[test]
fn a_failing_update_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    locked_at(dir.path(), "old");
    let ids = [skill_id(dir.path())];
    let runner = FakeRunner::new(|_| {}).failing("npx");
    let prompter = ScriptedPrompter::new(&[], true);
    let deps = Deps::for_test(dir.path(), registry(), (&runner, &prompter));

    let result = change(
        &session(&deps, dir.path()),
        Scope::Project,
        &Change::Update(&ids),
    );

    assert!(matches!(result.unwrap(), Some(Notice::Failed(_))));
}

#[test]
fn items_with_nothing_to_update_make_an_empty_plan() {
    let dir = tempfile::tempdir().unwrap();
    locked_at(dir.path(), "old");

    let (result, runner) = run_change(dir.path(), true, &Change::Update(&["nope".into()]));

    assert!(matches!(result.unwrap(), Some(Notice::UpToDate)));
    assert!(runner.calls().is_empty());
}

#[test]
fn an_update_records_the_new_version_in_the_library() {
    let dir = tempfile::tempdir().unwrap();
    locked_at(dir.path(), "old");
    let ids = [skill_id(dir.path())];
    let runner = FakeNpx::new();
    let prompter = ScriptedPrompter::new(&[], true);
    let deps = Deps::for_test(dir.path(), registry(), (&runner, &prompter));
    let store = Store::in_env(&project_env(&deps, dir.path()));

    change(
        &session(&deps, dir.path()),
        Scope::Project,
        &Change::Update(&ids),
    )
    .unwrap();

    let items = store.read().unwrap().items;
    let provenance = items.items()[0].provenance.clone().unwrap();
    assert_eq!(provenance.version.as_deref(), Some("abc123"));
}

#[test]
fn retrying_an_update_only_retries_the_failed_items() {
    use skillz_harness::runner::{CommandRunner, Output};
    struct FailsOnce {
        failed: std::cell::Cell<bool>,
        names: std::cell::RefCell<Vec<String>>,
    }
    impl CommandRunner for FailsOnce {
        fn run(&self, _: &str, args: &[String], _: &Path) -> anyhow::Result<Output> {
            let name = args[3].clone();
            self.names.borrow_mut().push(name.clone());
            let success = name != "b" || self.failed.replace(true);
            Ok(Output {
                success,
                stderr: String::new(),
            })
        }
    }
    let dir = tempfile::tempdir().unwrap();
    project_with_skill(dir.path(), "a");
    project_with_skill(dir.path(), "b");
    fs::write(
        dir.path().join("skills-lock.json"),
        r#"{"skills":{"a":{"source":"o/r"},"b":{"source":"o/r"}}}"#,
    )
    .unwrap();
    let runner = FailsOnce {
        failed: Default::default(),
        names: Default::default(),
    };
    let prompter = super::tests::Outcomes {
        retry_once: true,
        ..Default::default()
    };
    let deps = Deps::for_test(dir.path(), registry(), (&runner, &prompter));
    let ids: Vec<_> =
        crate::cli::scope::installed(&deps, &project_env(&deps, dir.path()), Scope::Project)
            .iter()
            .map(|i| i.id.clone())
            .collect();

    let result = change(
        &session(&deps, dir.path()),
        Scope::Project,
        &Change::Update(&ids),
    )
    .unwrap();

    assert_eq!(*runner.names.borrow(), ["a", "b", "b"]);
    assert!(matches!(result, Some(Notice::ItemsUpdated(1))));
}

#[test]
fn the_workspace_update_act_runs_the_same_update() {
    let dir = tempfile::tempdir().unwrap();
    locked_at(dir.path(), "old");
    let act = Act::Update {
        scope: Scope::Project,
        ids: vec![skill_id(dir.path())],
    };
    let runner = FakeRunner::new(|_| {});
    let prompter = ScriptedPrompter::new(&[], true);
    let deps = Deps::for_test(dir.path(), registry(), (&runner, &prompter));

    let result = session(&deps, dir.path()).perform(act);

    let calls = runner.calls();
    assert!(matches!(result.unwrap(), Some(Notice::ItemsUpdated(1))));
    assert_eq!(calls[0].1[2], "update");
}
