//! `skillz use` and `skillz profile ...`.

mod common;

use common::*;
use skillz::cli::{Command, Deps, ProfileCommand};
use skillz::harness::Spec;

fn world_with_web_and_api() -> World {
    let w = world();
    w.write_skill_profile("web", "tdd");
    w.write_skill_profile("api", "grill");
    w
}

fn world_with_web() -> World {
    let w = world();
    w.write_skill_profile("web", "tdd");
    w
}

fn profile_cmd(action: ProfileCommand) -> Command {
    Command::Profile {
        action,
        project: None,
    }
}

fn save(force: bool) -> Command {
    profile_cmd(ProfileCommand::Save {
        name: "mine".into(),
        force,
    })
}

fn delete_mine() -> Command {
    profile_cmd(ProfileCommand::Delete {
        name: "mine".into(),
    })
}

fn apply_all(d: &Deps, profiles: &[&str]) {
    for profile in profiles {
        run_quiet(use_cmd(profile, false), d).unwrap();
    }
}

#[test]
fn a_dry_run_use_runs_no_command() {
    let w = world_with_web();
    let (r, p) = (Recorder::default(), Scripted::yes());

    run_quiet(use_cmd("web", true), &deps(&w, &r, &p)).unwrap();

    assert_eq!(r.count(), 0);
}

#[test]
fn using_a_profile_installs_its_skills_once() {
    let w = world_with_web();
    let (r, p) = (Recorder::default(), Scripted::yes());

    apply_all(&deps(&w, &r, &p), &["web"]);

    assert_eq!(r.count(), 1);
}

#[test]
fn using_two_profiles_installs_each_ones_skills() {
    let w = world_with_web_and_api();
    let (r, p) = (Recorder::default(), Scripted::yes());

    apply_all(&deps(&w, &r, &p), &["web", "api"]);

    assert_eq!(r.count(), 2);
}

#[test]
fn using_a_missing_profile_fails() {
    let w = world_with_web();
    let (r, p) = (Recorder::default(), Scripted::yes());

    let res = run_quiet(use_cmd("missing", false), &deps(&w, &r, &p));

    assert!(res.is_err());
}

fn saved_profile() -> (World, String) {
    let w = world_with_project_setup();
    let (r, p) = (Recorder::default(), Scripted::yes());
    run_quiet(save(false), &deps(&w, &r, &p)).unwrap();
    let text = w.profile("mine").to_text().unwrap();
    (w, text)
}

#[test]
fn saving_snapshots_the_installed_skill_with_its_source() {
    let (w, _) = saved_profile();

    let entries = w.profile("mine").entries;

    assert!(entries.iter().any(|e| matches!(
        e,
        Spec::Skill { source, names, .. } if source == "mattpocock/skills" && names == &["tdd"]
    )));
}

#[test]
fn saving_snapshots_the_installed_hooks() {
    let (w, _) = saved_profile();

    let entries = w.profile("mine").entries;

    assert!(entries.iter().any(|e| matches!(e, Spec::Hook { .. })));
}

#[test]
fn saving_over_an_existing_profile_fails() {
    let (w, _) = saved_profile();
    let (r, p) = (Recorder::default(), Scripted::yes());

    let res = run_quiet(save(false), &deps(&w, &r, &p));

    assert!(res.is_err());
}

#[test]
fn saving_with_force_overwrites_the_profile() {
    let (w, _) = saved_profile();
    let (r, p) = (Recorder::default(), Scripted::yes());

    let res = run_quiet(save(true), &deps(&w, &r, &p));

    assert!(res.is_ok());
}

#[test]
fn a_shown_profile_can_be_imported_again() {
    let (w, _) = saved_profile();
    let (r, p) = (Recorder::default(), Scripted::yes());
    let d = deps(&w, &r, &p);
    let original = w.profile("mine");
    let text = run_captured(
        profile_cmd(ProfileCommand::Show {
            name: "mine".into(),
        }),
        &d,
    );
    let file = w.project.join("mine.toml");
    std::fs::write(&file, &text).unwrap();
    run_quiet(delete_mine(), &d).unwrap();

    run_quiet(profile_cmd(ProfileCommand::Import { file }), &d).unwrap();

    assert_eq!(w.profile("mine"), original);
}

#[test]
fn importing_a_file_that_is_not_a_valid_profile_stores_nothing() {
    let w = world_with_web();
    let (r, p) = (Recorder::default(), Scripted::yes());
    let file = w.project.join("bad.toml");
    std::fs::write(&file, "name = \"-bad\"").unwrap();
    let before = w.profile_names();

    let res = run_quiet(
        profile_cmd(ProfileCommand::Import { file }),
        &deps(&w, &r, &p),
    );

    assert!(res.is_err());
    assert_eq!(w.profile_names(), before);
}

#[test]
fn deleting_a_profile_removes_it_from_the_store() {
    let (w, _) = saved_profile();
    let (r, p) = (Recorder::default(), Scripted::yes());

    run_quiet(delete_mine(), &deps(&w, &r, &p)).unwrap();

    assert!(w.profile_names().is_empty());
}

#[test]
fn deleting_a_missing_profile_fails() {
    let (w, _) = saved_profile();
    let (r, p) = (Recorder::default(), Scripted::yes());
    let d = deps(&w, &r, &p);
    run_quiet(delete_mine(), &d).unwrap();

    let res = run_quiet(delete_mine(), &d);

    assert!(res.is_err());
}
