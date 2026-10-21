use std::fs;
use std::path::Path;

use super::*;
use crate::agents::registry;
use crate::install::test_support::{FakeRunner, ScriptedPrompter};
use crate::model::Agent;

/// A project with one skill installed the way `npx skills` leaves it.
fn project_with_skill(dir: &Path, name: &str) {
    let real = dir.join(".agents/skills").join(name);
    fs::create_dir_all(&real).unwrap();
    fs::write(
        real.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: d\n---\n"),
    )
    .unwrap();
    fs::create_dir_all(dir.join(".claude/skills")).unwrap();
    std::os::unix::fs::symlink(&real, dir.join(".claude/skills").join(name)).unwrap();
    let lock = serde_json::json!({"skills": {name: {"source": "o/r"}}});
    fs::write(dir.join("skills-lock.json"), lock.to_string()).unwrap();
}

fn skill_entry(name: &str) -> Entry {
    Entry::Skill {
        source: "o/r".into(),
        names: vec![name.into()],
        agents: vec![Agent::Claude],
    }
}

fn run_change(
    dir: &Path,
    approve: bool,
    what: &Change,
) -> (anyhow::Result<Option<Notice>>, FakeRunner) {
    let runner = FakeRunner::new(|_| {});
    let prompter = ScriptedPrompter::new(&[], approve);
    let deps = Deps::for_test(dir, registry(), (&runner, &prompter));
    let result = change(&deps, dir, what);
    (result, runner)
}

fn skill_id(dir: &Path) -> String {
    let runner = FakeRunner::new(|_| {});
    let prompter = ScriptedPrompter::new(&[], true);
    let deps = Deps::for_test(dir, registry(), (&runner, &prompter));
    let env = project_env(&deps, dir);
    installed(&deps, &env, Scope::Project)[0].id.clone()
}

#[test]
fn removing_an_approved_item_deletes_it_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    project_with_skill(dir.path(), "tdd");
    let id = skill_id(dir.path());

    let (result, _) = run_change(dir.path(), true, &Change::Remove(&id));

    assert!(matches!(result.unwrap(), Some(Notice::Removed(_))));
    assert!(!dir.path().join(".agents/skills/tdd").exists());
}

#[test]
fn a_declined_plan_changes_nothing_and_reports_nothing() {
    let dir = tempfile::tempdir().unwrap();
    project_with_skill(dir.path(), "tdd");
    let id = skill_id(dir.path());

    let (result, _) = run_change(dir.path(), false, &Change::Remove(&id));

    assert!(result.unwrap().is_none());
    assert!(dir.path().join(".agents/skills/tdd").exists());
}

#[test]
fn removing_something_already_gone_is_an_error() {
    let dir = tempfile::tempdir().unwrap();

    let (result, _) = run_change(dir.path(), true, &Change::Remove("skill:gone"));

    assert!(result.is_err());
}

#[test]
fn installing_an_entry_runs_its_install_steps() {
    let dir = tempfile::tempdir().unwrap();
    let entry = skill_entry("tdd");

    let (result, runner) = run_change(dir.path(), true, &Change::Install(&entry));

    assert!(matches!(result.unwrap(), Some(Notice::Installed(_))));
    assert!(
        runner
            .calls()
            .iter()
            .any(|(_, args, _)| args.contains(&"add".to_string()))
    );
}

#[test]
fn installing_what_is_already_there_needs_no_plan() {
    let dir = tempfile::tempdir().unwrap();
    project_with_skill(dir.path(), "tdd");
    let entry = skill_entry("tdd");

    let (result, runner) = run_change(dir.path(), true, &Change::Install(&entry));

    assert!(matches!(result.unwrap(), Some(Notice::UpToDate)));
    assert!(runner.calls().is_empty());
}

#[test]
fn applying_a_profile_installs_what_it_has_that_the_project_lacks() {
    let dir = tempfile::tempdir().unwrap();
    let mut profile = Profile::new("web");
    profile.agents = vec![Agent::Claude];
    profile.entries = vec![skill_entry("tdd")];
    let env = Env::new(dir.path().into(), dir.path().into());
    profile::Store::in_env(&env).save(&profile).unwrap();

    let (result, runner) = run_change(dir.path(), true, &Change::Apply("web"));

    assert!(matches!(result.unwrap(), Some(Notice::Applied(_))));
    assert!(!runner.calls().is_empty());
}

#[test]
fn a_failed_run_is_a_failure_and_a_clean_one_is_not() {
    let switch = Switch {
        to: "p".into(),
        present: vec![],
        removals: vec![],
        unsourced: vec![],
        installs: vec![],
        unsupported: vec![],
    };

    let failed = outcome_of(&switch, Err(anyhow::anyhow!("boom")));
    let clean = outcome_of(
        &switch,
        Ok(Done {
            removed: None,
            installed: Default::default(),
        }),
    );

    assert_eq!((failed.is_clean(), clean.is_clean()), (false, true));
}
