use std::fs;
use std::path::Path;

use super::*;
use crate::app::install::test_support::{FakeRunner, ScriptedPrompter};
use crate::app::profile::{self, StillExtended};
use crate::app::store;
use crate::cli::deps::Deps;
use crate::cli::profile::save_profile;
use crate::cli::scope::{complete_installed, project_env};
use skillz_harness::agents::registry;
use skillz_harness::model::{Agent, Scope};

fn session<'a, 'd>(deps: &'a Deps<'d>, dir: &Path) -> Session<'a, 'd> {
    Session {
        deps,
        profiles: profile::Store::new(dir.join("store.json")),
        store: store::Store::new(dir.join("store.json")),
        project: dir.to_path_buf(),
    }
}

fn with_session<T>(check: impl FnOnce(&Session, &Path) -> T) -> T {
    let dir = tempfile::tempdir().unwrap();
    let runner = FakeRunner::new(|_| {});
    let prompter = ScriptedPrompter::new(&[], true);
    let deps = Deps::for_test(dir.path(), registry(), (&runner, &prompter));
    check(&session(&deps, dir.path()), dir.path())
}

/// What `skillz profile save` does: snapshot the project's installs into a profile.
fn save_current(session: &Session, name: &str) -> anyhow::Result<crate::app::snapshot::Snapshot> {
    let env = project_env(session.deps, &session.project);
    let items = complete_installed(session.deps, &env, Scope::Project)?;
    save_profile(&session.profiles, &items, name, false)
}

fn skill(name: &str) -> Spec {
    Spec::Skill {
        source: "o/r".into(),
        names: vec![name.into()],
        agents: vec![Agent::Claude],
    }
}

#[test]
fn a_created_profile_is_stored_empty_and_cannot_be_created_twice() {
    with_session(|session, _| {
        let first = session.create_profile("web", &[]);
        let again = session.create_profile("web", &[]);

        assert!(first.is_ok() && again.is_err());
        assert!(session.profiles.load("web").unwrap().entries.is_empty());
    });
}

#[test]
fn setting_entries_replaces_the_stored_ones() {
    with_session(|session, _| {
        session.create_profile("web", &[]).unwrap();

        session.set_entries("web", &[skill("tdd")]).unwrap();
        session.set_entries("web", &[skill("grill")]).unwrap();

        assert_eq!(
            session.profiles.load("web").unwrap().entries,
            [skill("grill")]
        );
    });
}

#[test]
fn deleting_a_profile_others_extend_fails_with_the_typed_error() {
    with_session(|session, _| {
        session.create_profile("base", &[]).unwrap();
        let mut web = Profile::new("web");
        web.extends = vec!["base".into()];
        session.profiles.save(&web).unwrap();

        let error = session.delete_profile("base").unwrap_err();

        assert!(error.downcast_ref::<StillExtended>().is_some());
        assert!(session.profiles.load("base").is_ok());
    });
}

#[test]
fn deleting_removes_the_profile() {
    with_session(|session, _| {
        session.create_profile("web", &[]).unwrap();

        session.delete_profile("web").unwrap();

        assert!(session.profiles.names().unwrap().is_empty());
    });
}

#[test]
fn saving_the_project_lists_what_could_not_be_saved() {
    with_session(|session, dir| {
        let loose = dir.join(".claude/skills/mine");
        fs::create_dir_all(&loose).unwrap();
        fs::write(loose.join("SKILL.md"), "---\nname: mine\n---\n").unwrap();

        let saved = save_current(session, "snap").unwrap();

        assert_eq!(saved.skipped.len(), 1);
        assert!(session.profiles.load("snap").unwrap().entries.is_empty());
    });
}

#[test]
fn saving_the_project_keeps_a_hand_made_skill_by_the_dir_holding_it() {
    with_session(|session, dir| {
        let loose = dir.join(".claude/skills/mine");
        fs::create_dir_all(&loose).unwrap();
        fs::write(
            loose.join("SKILL.md"),
            "---\nname: mine\ndescription: d\n---\n",
        )
        .unwrap();

        save_current(session, "snap").unwrap();

        // Assert the profile reinstalls it from the skills dir, canonicalized as discovery does.
        let held_in = fs::canonicalize(dir.join(".claude/skills")).unwrap();
        assert_eq!(
            session.profiles.load("snap").unwrap().entries,
            [Spec::Skill {
                source: held_in.to_string_lossy().into(),
                names: vec!["mine".into()],
                agents: vec![Agent::Claude],
            }]
        );
    });
}

#[test]
fn saving_over_an_existing_profile_is_refused() {
    with_session(|session, _| {
        session.create_profile("snap", &[]).unwrap();

        assert!(save_current(session, "snap").is_err());
    });
}

#[test]
fn the_applied_profile_is_remembered_per_project_and_replaced_by_the_next() {
    with_session(|here, dir| {
        let elsewhere = Session {
            project: dir.join("other"),
            ..session(here.deps, dir)
        };

        let before = here.applied(Scope::Project);
        here.remember_applied(Scope::Project, "web").unwrap();
        here.remember_applied(Scope::Project, "api").unwrap();

        assert_eq!(before, None);
        assert_eq!(here.applied(Scope::Project).as_deref(), Some("api"));
        assert_eq!(elsewhere.applied(Scope::Project), None);
    });
}

#[test]
fn the_user_level_remembers_its_own_applied_profile_apart_from_every_project() {
    with_session(|here, _| {
        here.remember_applied(Scope::Project, "web").unwrap();
        here.remember_applied(Scope::Global, "api").unwrap();

        assert_eq!(here.applied(Scope::Project).as_deref(), Some("web"));
        assert_eq!(here.applied(Scope::Global).as_deref(), Some("api"));
    });
}

#[test]
fn deleting_a_profile_forgets_it_as_the_applied_one() {
    with_session(|here, _| {
        here.create_profile("web", &[]).unwrap();
        here.remember_applied(Scope::Project, "web").unwrap();

        here.delete_profile("web").unwrap();
        here.create_profile("web", &[]).unwrap();

        assert_eq!(here.applied(Scope::Project), None);
    });
}
