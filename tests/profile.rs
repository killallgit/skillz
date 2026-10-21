//! `skillz use`, `skillz profile ...` and `skillz project ...`.

mod common;

use common::*;
use skillz::cli::{Command, Deps, ProfileCommand, ProjectCommand};
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
    Command::Profile { action }
}

fn project_cmd(action: ProjectCommand) -> Command {
    Command::Project { action }
}

fn save(force: bool) -> Command {
    profile_cmd(ProfileCommand::Save {
        name: "mine".into(),
        global: true,
        project: None,
        force,
    })
}

fn delete_mine() -> Command {
    profile_cmd(ProfileCommand::Delete {
        name: "mine".into(),
    })
}

fn apply_all(d: &Deps, profiles: &[Option<&str>]) {
    for profile in profiles {
        run_quiet(use_cmd(*profile, false), d).unwrap();
    }
}

fn listed_project_lines(d: &Deps) -> usize {
    let out = run_captured(project_cmd(ProjectCommand::List), d);
    String::from_utf8(out).unwrap().lines().count()
}

// --- use

#[test]
fn a_dry_run_use_runs_no_command() {
    let w = world_with_web();
    let (r, p) = (Recorder::default(), Scripted::yes());

    run_quiet(use_cmd(Some("web"), true), &deps(&w, &r, &p)).unwrap();

    assert_eq!(r.count(), 0);
}

#[test]
fn a_dry_run_use_records_nothing() {
    let w = world_with_web();
    let (r, p) = (Recorder::default(), Scripted::yes());

    run_quiet(use_cmd(Some("web"), true), &deps(&w, &r, &p)).unwrap();

    assert!(!w.ledger_exists());
}

#[test]
fn using_a_profile_installs_its_skills_once() {
    let w = world_with_web();
    let (r, p) = (Recorder::default(), Scripted::yes());

    apply_all(&deps(&w, &r, &p), &[Some("web")]);

    assert_eq!(r.count(), 1);
}

#[test]
fn using_a_profile_records_it_in_the_ledger() {
    let w = world_with_web();
    let (r, p) = (Recorder::default(), Scripted::yes());

    apply_all(&deps(&w, &r, &p), &[Some("web")]);

    assert_eq!(w.applied_profiles(), ["web"]);
}

#[test]
fn using_the_same_profile_again_installs_nothing() {
    let w = world_with_web();
    let (r, p) = (Recorder::default(), Scripted::yes());

    apply_all(&deps(&w, &r, &p), &[Some("web"), Some("web")]);

    assert_eq!(r.count(), 1);
}

#[test]
fn switching_profiles_installs_only_the_new_profiles_skills() {
    let w = world_with_web_and_api();
    let (r, p) = (Recorder::default(), Scripted::yes());

    apply_all(&deps(&w, &r, &p), &[Some("web"), Some("api")]);

    assert_eq!(r.count(), 2);
}

#[test]
fn switching_profiles_replaces_the_recorded_profile() {
    let w = world_with_web_and_api();
    let (r, p) = (Recorder::default(), Scripted::yes());

    apply_all(&deps(&w, &r, &p), &[Some("web"), Some("api")]);

    assert_eq!(w.applied_profiles(), ["api"]);
}

#[test]
fn using_none_forgets_the_applied_profile() {
    let w = world_with_web_and_api();
    let (r, p) = (Recorder::default(), Scripted::yes());

    apply_all(&deps(&w, &r, &p), &[Some("api"), None]);

    assert!(w.applied_profiles().is_empty());
}

#[test]
fn using_a_missing_profile_fails() {
    let w = world_with_web();
    let (r, p) = (Recorder::default(), Scripted::yes());

    let res = run_quiet(use_cmd(Some("missing"), false), &deps(&w, &r, &p));

    assert!(res.is_err());
}

// --- profile save / show / list / delete

fn saved_profile() -> (World, String) {
    let w = world();
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

// --- projects

#[test]
fn no_project_is_registered_at_first() {
    let w = world_with_web();
    let (r, p) = (Recorder::default(), Scripted::yes());

    assert_eq!(listed_project_lines(&deps(&w, &r, &p)), 0);
}

#[test]
fn using_a_profile_registers_the_project() {
    let w = world_with_web();
    let (r, p) = (Recorder::default(), Scripted::yes());

    apply_all(&deps(&w, &r, &p), &[Some("web")]);

    assert_eq!(w.registered_projects(), [w.project.canonicalize().unwrap()]);
}

#[test]
fn the_registered_project_is_listed() {
    let w = world_with_web();
    let (r, p) = (Recorder::default(), Scripted::yes());
    let d = deps(&w, &r, &p);
    apply_all(&d, &[Some("web")]);

    assert_eq!(listed_project_lines(&d), 1);
}

#[test]
fn removing_a_project_unregisters_it() {
    let w = world_with_web();
    let (r, p) = (Recorder::default(), Scripted::yes());
    let d = deps(&w, &r, &p);
    apply_all(&d, &[Some("web")]);

    run_quiet(project_cmd(ProjectCommand::Remove { path: ".".into() }), &d).unwrap();

    assert!(w.registered_projects().is_empty());
}

#[test]
fn removing_an_unregistered_project_fails() {
    let w = world_with_web();
    let (r, p) = (Recorder::default(), Scripted::yes());

    let res = run_quiet(
        project_cmd(ProjectCommand::Remove { path: ".".into() }),
        &deps(&w, &r, &p),
    );

    assert!(res.is_err());
}

#[test]
fn adding_without_a_path_registers_the_cwd() {
    let w = world_with_web();
    let (r, p) = (Recorder::default(), Scripted::yes());

    run_quiet(
        project_cmd(ProjectCommand::Add { path: None }),
        &deps(&w, &r, &p),
    )
    .unwrap();

    assert_eq!(w.registered_projects(), [w.project.canonicalize().unwrap()]);
}

#[test]
fn a_forgotten_project_still_has_its_profile_in_the_ledger() {
    let w = world_with_web();
    let (r, p) = (Recorder::default(), Scripted::yes());
    let d = deps(&w, &r, &p);
    apply_all(&d, &[Some("web")]);

    run_quiet(project_cmd(ProjectCommand::Remove { path: ".".into() }), &d).unwrap();

    assert_eq!(w.applied_profiles(), ["web"]);
}
