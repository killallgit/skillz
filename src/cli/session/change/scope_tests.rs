use std::fs;
use std::path::Path;

use super::tests::{project_with_skill, session, skill_entry};
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

    // The install runs after the plan fetched the source, so it is the last command.
    assert!(global.last().unwrap().contains(&"-g".to_string()));
    assert!(!local.last().unwrap().contains(&"-g".to_string()));
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
        ["codex"]
    );
}

#[test]
fn codex_library_and_profile_plugins_install_only_in_the_project() {
    use skillz_harness::model::{Agent, Kind};
    let entry = skillz_harness::Spec::Plugin {
        source: "mattpocock/skills".into(),
        marketplace: "mattpocock".into(),
        name: "mattpocock-skills".into(),
        agents: vec![Agent::Codex],
    };
    for from_profile in [false, true] {
        let (home, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let global = home.path().join(".codex/config.toml");
        let original = "model = \"o3\"\n[plugins.\"existing@mp\"]\nenabled = true\n";
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::write(&global, original).unwrap();
        let env = Env::new(home.path().into(), project.path().into());
        let mut profile = profile::Profile::new("matt");
        profile.entries = vec![entry.clone()];
        profile::Store::in_env(&env).save(&profile).unwrap();
        let selection = Selection {
            install: std::slice::from_ref(&entry),
            remove: &[],
            only: None,
        };
        let what = if from_profile {
            Change::Apply("matt")
        } else {
            Change::Selection(&selection)
        };

        let calls = run_in(home.path(), project.path(), Scope::Project, &what);

        assert_eq!(calls.len(), 1);
        assert!(
            calls[0]
                .windows(4)
                .any(|args| args == ["plugin", "marketplace", "upgrade", "mattpocock"])
        );
        let config: toml_edit::DocumentMut =
            fs::read_to_string(project.path().join(".codex/config.toml"))
                .unwrap()
                .parse()
                .unwrap();
        assert_eq!(
            config["marketplaces"]["mattpocock"]["source"].as_str(),
            Some("https://github.com/mattpocock/skills.git")
        );
        assert_eq!(
            config["plugins"]["mattpocock-skills@mattpocock"]["enabled"].as_bool(),
            Some(true)
        );
        assert_eq!(fs::read_to_string(&global).unwrap(), original);
        let adapters = registry();
        let installed = skillz_harness::agents::inventory(&adapters, &env, &[Scope::Project])
            .complete()
            .unwrap();
        assert!(installed.iter().any(|item| item.kind == Kind::Plugin
            && item.name == "mattpocock-skills@mattpocock"
            && item.source.as_deref() == Some("mattpocock/skills")));
        assert!(run_in(home.path(), project.path(), Scope::Project, &what).is_empty());
    }
}

#[test]
fn replacing_at_user_level_removes_user_installs_and_leaves_the_project_alone() {
    let (home, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    global_skill(home.path(), "userskill");
    project_with_skill(project.path(), "projectskill");
    let env = Env::new(home.path().into(), project.path().into());
    let mut profile = profile::Profile::new("web");
    profile.agents = vec![skillz_harness::model::Agent::Claude];
    profile::Store::in_env(&env).save(&profile).unwrap();
    let runner = FakeRunner::new(|_| {});
    let prompter = ScriptedPrompter::new(&[], true);
    let deps = Deps::for_test(home.path(), registry(), (&runner, &prompter));

    let applied = change(
        &session(&deps, project.path()),
        Scope::Global,
        &Change::ApplyExact("web"),
    )
    .unwrap();

    let argv: Vec<String> = runner
        .calls()
        .into_iter()
        .flat_map(|(_, args, _)| args)
        .collect();
    let project_names: Vec<String> = installed(&deps, &env, Scope::Project)
        .into_iter()
        .map(|item| item.name)
        .collect();
    assert!(matches!(applied, Some(Notice::Applied(_))));
    assert!(argv.iter().any(|arg| arg == "userskill"));
    assert!(!argv.iter().any(|arg| arg == "projectskill"));
    assert!(project_names.iter().any(|name| name == "projectskill"));
}
