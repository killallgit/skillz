use std::fs;
use std::path::Path;

use super::tests::{project_with_skill, run_change, session, skill_id};
use super::*;
use crate::agents::registry;
use crate::install::test_support::{FakeNpx, FakeRunner, ScriptedPrompter};

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
