use std::fs;
use std::path::Path;

use super::*;
use crate::agents::registry;
use crate::cli::scope::installed;
use crate::install::test_support::{FakeRunner, ScriptedPrompter};
use crate::model::Agent;
use crate::profile::{Entry, Profile};

/// A project with one skill installed the way `npx skills` leaves it.
pub(super) fn project_with_skill(dir: &Path, name: &str) {
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

pub(super) fn skill_entry(name: &str) -> Entry {
    Entry::Skill {
        source: "o/r".into(),
        names: vec![name.into()],
        agents: vec![Agent::Claude],
    }
}

pub(super) fn session<'a, 'd>(deps: &'a Deps<'d>, project: &Path) -> Session<'a, 'd> {
    let env = project_env(deps, project);
    Session {
        deps,
        profiles: profile::Store::in_env(&env),
        store: Store::in_env(&env),
        project: project.to_path_buf(),
    }
}

pub(super) fn run_change(
    dir: &Path,
    approve: bool,
    what: &Change,
) -> (anyhow::Result<Option<Notice>>, FakeRunner) {
    let runner = FakeRunner::new(|_| {});
    let prompter = ScriptedPrompter::new(&[], approve);
    let deps = Deps::for_test(dir, registry(), (&runner, &prompter));
    let result = change(&session(&deps, dir), Scope::Project, what);
    (result, runner)
}

pub(super) fn skill_id(dir: &Path) -> String {
    let runner = FakeRunner::new(|_| {});
    let prompter = ScriptedPrompter::new(&[], true);
    let deps = Deps::for_test(dir, registry(), (&runner, &prompter));
    let env = project_env(&deps, dir);
    installed(&deps, &env, Scope::Project)[0].id.clone()
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

fn save_empty_profile(dir: &Path, name: &str) {
    let mut profile = Profile::new(name);
    profile.agents = vec![Agent::Claude];
    let env = Env::new(dir.into(), dir.into());
    profile::Store::in_env(&env).save(&profile).unwrap();
}

#[test]
fn applying_exactly_removes_what_the_profile_does_not_have() {
    let dir = tempfile::tempdir().unwrap();
    project_with_skill(dir.path(), "tdd");
    save_empty_profile(dir.path(), "web");

    let (result, _) = run_change(dir.path(), true, &Change::ApplyExact("web"));

    assert!(matches!(result.unwrap(), Some(Notice::Applied(_))));
    assert!(!dir.path().join(".agents/skills/tdd").exists());
}

#[test]
fn applying_additively_keeps_what_the_profile_does_not_have() {
    let dir = tempfile::tempdir().unwrap();
    project_with_skill(dir.path(), "tdd");
    save_empty_profile(dir.path(), "web");

    let (result, _) = run_change(dir.path(), true, &Change::Apply("web"));

    assert!(matches!(result.unwrap(), Some(Notice::UpToDate)));
    assert!(dir.path().join(".agents/skills/tdd").exists());
}

#[test]
fn a_failed_run_is_a_failure_and_a_clean_one_is_not() {
    let switch = Switch::updating(vec![]);

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
