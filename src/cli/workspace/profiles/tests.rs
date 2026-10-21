use std::fs;
use std::path::Path;

use super::*;
use crate::agents::registry;
use crate::cli::deps::Deps;
use crate::install::test_support::{FakeRunner, ScriptedPrompter};
use crate::model::Agent;
use crate::profile::{self, StillExtended};
use crate::store;

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

fn skill(name: &str) -> Entry {
    Entry::Skill {
        source: "o/r".into(),
        names: vec![name.into()],
        agents: vec![Agent::Claude],
    }
}

#[test]
fn a_created_profile_is_stored_empty_and_cannot_be_created_twice() {
    with_session(|session, _| {
        let first = session.create_profile("web");
        let again = session.create_profile("web");

        assert!(first.is_ok() && again.is_err());
        assert!(session.profiles.load("web").unwrap().entries.is_empty());
    });
}

#[test]
fn setting_entries_replaces_the_stored_ones() {
    with_session(|session, _| {
        session.create_profile("web").unwrap();

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
        session.create_profile("base").unwrap();
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
        session.create_profile("web").unwrap();

        session.delete_profile("web").unwrap();

        assert!(session.profiles.names().unwrap().is_empty());
    });
}

#[test]
fn saving_the_project_lists_what_could_not_be_saved() {
    with_session(|session, dir| {
        let loose = dir.join(".claude/skills/mine");
        fs::create_dir_all(&loose).unwrap();
        fs::write(
            loose.join("SKILL.md"),
            "---\nname: mine\ndescription: d\n---\n",
        )
        .unwrap();

        let notice = session.save_current("snap").unwrap();

        let Notice::Saved { skipped, .. } = notice else {
            panic!("expected a saved notice");
        };
        assert_eq!(skipped.len(), 1);
        assert!(session.profiles.load("snap").unwrap().entries.is_empty());
    });
}

#[test]
fn saving_over_an_existing_profile_is_refused() {
    with_session(|session, _| {
        session.create_profile("snap").unwrap();

        assert!(session.save_current("snap").is_err());
    });
}
