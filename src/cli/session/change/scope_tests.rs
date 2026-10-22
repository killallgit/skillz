use std::fs;
use std::path::Path;

use super::tests::{session, skill_entry};
use super::*;
use crate::agents::registry;
use crate::cli::scope::installed;
use crate::install::test_support::{FakeRunner, ScriptedPrompter};

/// A skill in the user's own directories, the way `npx skills -g` leaves it.
fn global_skill(home: &Path, name: &str) {
    let real = home.join(".agents/skills").join(name);
    fs::create_dir_all(&real).unwrap();
    fs::write(
        real.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: d\n---\n"),
    )
    .unwrap();
    fs::create_dir_all(home.join(".claude/skills")).unwrap();
    std::os::unix::fs::symlink(&real, home.join(".claude/skills").join(name)).unwrap();
    let lock = serde_json::json!({"version": 3, "skills": {name: {"source": "o/r"}}});
    fs::write(home.join(".agents/.skill-lock.json"), lock.to_string()).unwrap();
}

/// The arguments of every command the change ran.
fn run_in(home: &Path, project: &Path, scope: Scope, what: &Change) -> Vec<Vec<String>> {
    let runner = FakeRunner::new(|_| {});
    let prompter = ScriptedPrompter::new(&[], true);
    let deps = Deps::for_test(home, registry(), (&runner, &prompter));
    change(&session(&deps, project), scope, what).unwrap();
    runner
        .calls()
        .into_iter()
        .map(|(_, args, _)| args)
        .collect()
}

#[test]
fn installing_at_global_scope_gives_npx_the_global_flag_and_project_scope_does_not() {
    let (home, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let entries = [skill_entry("tdd")];
    let picked = Selection {
        install: &entries,
        remove: &[],
    };

    let global = run_in(
        home.path(),
        project.path(),
        Scope::Global,
        &Change::Selection(&picked),
    );
    let local = run_in(
        home.path(),
        project.path(),
        Scope::Project,
        &Change::Selection(&picked),
    );

    assert!(global[0].contains(&"-g".to_string()));
    assert!(!local[0].contains(&"-g".to_string()));
}

#[test]
fn updating_at_global_scope_finds_global_items_and_updates_them_globally() {
    let (home, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    global_skill(home.path(), "tdd");
    let runner = FakeRunner::new(|_| {});
    let prompter = ScriptedPrompter::new(&[], true);
    let deps = Deps::for_test(home.path(), registry(), (&runner, &prompter));
    let env = project_env(&deps, project.path());
    let ids = [installed(&deps, &env, Scope::Global)[0].id.clone()];

    let global = run_in(
        home.path(),
        project.path(),
        Scope::Global,
        &Change::Update(&ids),
    );
    let local = run_in(
        home.path(),
        project.path(),
        Scope::Project,
        &Change::Update(&ids),
    );

    assert_eq!(global.len(), 1);
    assert!(global[0].contains(&"-g".to_string()));
    assert!(local.is_empty());
}

#[test]
fn removing_at_global_scope_removes_only_the_global_item() {
    let (home, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    global_skill(home.path(), "tdd");
    let runner = FakeRunner::new(|_| {});
    let prompter = ScriptedPrompter::new(&[], true);
    let deps = Deps::for_test(home.path(), registry(), (&runner, &prompter));
    let env = project_env(&deps, project.path());
    let id = installed(&deps, &env, Scope::Global)[0].id.clone();
    let picked = Selection {
        install: &[],
        remove: &[id],
    };

    let wrong_scope = change(
        &session(&deps, project.path()),
        Scope::Project,
        &Change::Selection(&picked),
    );
    let removed = change(
        &session(&deps, project.path()),
        Scope::Global,
        &Change::Selection(&picked),
    );

    assert!(matches!(wrong_scope.unwrap(), Some(Notice::UpToDate)));
    assert!(matches!(
        removed.unwrap(),
        Some(Notice::Changed { removed: 1, .. })
    ));
}
