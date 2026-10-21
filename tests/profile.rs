//! `skillz use` and `skillz profile ...`.

mod common;

use common::*;
use skillz::cli::{Command, Deps, ProfileCommand};
use skillz::profile::Entry;

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

// --- use

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

// --- profile save / show / list / delete

fn saved_profile() -> (World, String) {
    let w = world_with_project_setup();
    let (r, p) = (Recorder::default(), Scripted::yes());
    run_quiet(save(false), &deps(&w, &r, &p)).unwrap();
    let text = std::fs::read_to_string(w.skillz_dir().join("profiles/mine.toml")).unwrap();
    (w, text)
}

#[test]
fn saving_snapshots_the_installed_skill_with_its_source() {
    let (w, _) = saved_profile();

    let entries = w.profile("mine").entries;

    assert!(entries.iter().any(|e| matches!(
        e,
        Entry::Skill { source, names, .. } if source == "mattpocock/skills" && names == &["tdd"]
    )));
}

#[test]
fn saving_snapshots_the_installed_hooks() {
    let (w, _) = saved_profile();

    let entries = w.profile("mine").entries;

    assert!(entries.iter().any(|e| matches!(e, Entry::Hook { .. })));
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
fn a_saved_profile_is_in_the_store() {
    let (w, _) = saved_profile();

    assert_eq!(w.profile_names(), ["mine"]);
}

#[test]
fn profile_list_prints_one_line_per_profile() {
    let (w, _) = saved_profile();
    let (r, p) = (Recorder::default(), Scripted::yes());

    let out = run_captured(profile_cmd(ProfileCommand::List), &deps(&w, &r, &p));

    assert_eq!(String::from_utf8(out).unwrap().lines().count(), 1);
}

#[test]
fn profile_show_prints_the_stored_toml() {
    let (w, text) = saved_profile();
    let (r, p) = (Recorder::default(), Scripted::yes());
    let show = profile_cmd(ProfileCommand::Show {
        name: "mine".into(),
    });

    let out = run_captured(show, &deps(&w, &r, &p));

    assert_eq!(String::from_utf8(out).unwrap(), text);
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
