use std::fs;
use std::path::Path;

use super::tests::{session, skill_entry};
use super::*;
use crate::app::install::test_support::{FakeRunner, ScriptedPrompter};
use crate::cli::scope::installed;
use skillz_harness::agents::registry;

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
        only: None,
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
        only: None,
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

#[test]
fn applying_a_plugin_profile_scopes_both_marketplace_and_plugin_commands() {
    let (home, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let env = Env::new(home.path().into(), project.path().into());
    let mut profile = profile::Profile::new("plugins");
    profile.entries = vec![skillz_harness::Spec::Plugin {
        source: "o/r".into(),
        marketplace: "mp".into(),
        name: "p".into(),
        agents: vec![skillz_harness::model::Agent::Claude],
    }];
    profile::Store::in_env(&env).save(&profile).unwrap();
    for (scope, native_scope) in [(Scope::Project, "local"), (Scope::Global, "user")] {
        let calls = run_in(
            home.path(),
            project.path(),
            scope,
            &Change::Apply("plugins"),
        );
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls[0],
            [
                "plugin",
                "marketplace",
                "add",
                "o/r",
                "--scope",
                native_scope
            ]
        );
        assert_eq!(
            calls[1],
            ["plugin", "install", "p@mp", "--scope", native_scope]
        );
    }
}

#[test]
fn installing_a_library_entry_goes_to_the_chosen_scope_and_only_the_chosen_tools() {
    use skillz_harness::model::Agent;
    let plugin = |agents: Vec<Agent>| skillz_harness::Spec::Plugin {
        source: "o/r".into(),
        marketplace: "mp".into(),
        name: "p".into(),
        agents,
    };
    let programs = |entry, scope| {
        let (home, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let runner = FakeRunner::new(|_| {});
        let prompter = ScriptedPrompter::new(&[], true);
        let deps = Deps::for_test(home.path(), registry(), (&runner, &prompter));
        let picked = Selection {
            install: &[entry],
            remove: &[],
            only: None,
        };
        change(
            &session(&deps, project.path()),
            scope,
            &Change::Selection(&picked),
        )
        .unwrap();
        runner
            .calls()
            .into_iter()
            .map(|(program, _, _)| program)
            .collect::<Vec<_>>()
    };

    assert_eq!(
        programs(plugin(vec![Agent::Claude]), Scope::Global),
        ["claude", "claude"]
    );
    assert_eq!(
        programs(plugin(vec![Agent::Claude, Agent::Codex]), Scope::Global),
        ["claude", "claude", "codex", "codex"]
    );
    assert_eq!(
        programs(plugin(vec![Agent::Codex]), Scope::Project),
        Vec::<String>::new()
    );
}
