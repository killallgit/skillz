//! `skillz profile new/add/remove/list/status`: the Profiles tab's features from the command
//! line, through the binary.

mod common;

use common::*;
use serde_json::Value;
use skillz::profile::Entry;

fn skill(name: &str) -> Entry {
    Entry::Skill {
        source: "o/r".into(),
        names: vec![name.into()],
        agents: vec![],
    }
}

fn with_profile(name: &str) -> Sandbox {
    let sb = Sandbox::new();
    assert!(sb.skillz(&["profile", "new", name]).status.success());
    sb
}

fn row<'a>(rows: &'a Value, name: &str) -> &'a Value {
    let rows = rows.as_array().unwrap();
    rows.iter().find(|r| r["name"] == name).unwrap()
}

/// `a` holds one hook and `b` another, with `a` loaded into the project.
fn with_a_loaded_and_b_not() -> Sandbox {
    let sb = Sandbox::new();
    sb.write_profile("a", &hook_profile("Stop", "say a", "\"claude\""));
    sb.write_profile("b", &hook_profile("Stop", "say b", "\"claude\""));
    assert!(sb.skillz(&["use", "-y", "a"]).status.success());
    sb
}

#[test]
fn a_new_profile_is_empty_and_cannot_be_created_twice() {
    let sb = Sandbox::new();

    let first = sb.skillz(&["profile", "new", "web"]);
    let again = sb.skillz(&["profile", "new", "web"]);

    assert!(first.status.success() && !again.status.success());
    assert!(sb.profile("web").entries.is_empty());
}

#[test]
fn a_typed_line_is_added_to_the_profile_once() {
    let sb = with_profile("web");

    sb.skillz(&["profile", "add", "web", "skill o/r tdd"]);
    let again = sb.skillz(&["profile", "add", "web", "skill o/r tdd"]);

    assert!(again.status.success());
    assert_eq!(sb.profile("web").entries, [skill("tdd")]);
}

#[test]
fn a_line_with_a_secret_is_refused_and_a_dry_run_adds_nothing() {
    let sb = with_profile("web");

    let secret = sb.skillz(&["profile", "add", "web", "hook Stop run --token abc"]);
    let dry = sb.skillz(&["profile", "add", "web", "skill o/r tdd", "--dry-run"]);

    assert!(!secret.status.success() && dry.status.success());
    assert!(sb.profile("web").entries.is_empty());
}

#[test]
fn library_items_are_copied_into_the_profile() {
    let sb = with_profile("web");
    sb.skillz(&["library", "add", "skill o/r tdd"]);
    sb.skillz(&["library", "add", "skill o/r grill"]);

    let out = sb.skillz(&["profile", "add", "web", "--from-library", "tdd", "-y"]);

    assert!(out.status.success());
    assert_eq!(sb.profile("web").entries, [skill("tdd")]);
}

#[test]
fn a_query_the_library_cannot_answer_changes_nothing() {
    let sb = with_profile("web");

    let out = sb.skillz(&["profile", "add", "web", "--from-library", "nope", "-y"]);

    assert!(!out.status.success());
    assert!(sb.profile("web").entries.is_empty());
}

#[test]
fn adding_to_a_profile_that_does_not_exist_fails() {
    let sb = Sandbox::new();

    let out = sb.skillz(&["profile", "add", "ghost", "skill o/r tdd"]);

    assert!(!out.status.success());
}

#[test]
fn removing_takes_the_matching_entry_out_and_keeps_the_rest() {
    let sb = with_profile("web");
    sb.skillz(&["profile", "add", "web", "skill o/r tdd"]);
    sb.skillz(&["profile", "add", "web", "skill o/r grill"]);

    let out = sb.skillz(&["profile", "remove", "web", "tdd", "-y"]);

    assert!(out.status.success());
    assert_eq!(sb.profile("web").entries, [skill("grill")]);
}

#[test]
fn an_ambiguous_removal_is_refused_with_yes_and_a_dry_run_removes_nothing() {
    let sb = with_profile("web");
    sb.skillz(&["profile", "add", "web", "skill o/r tdd"]);
    sb.skillz(&["profile", "add", "web", "skill o/r tdd-extra"]);

    let ambiguous = sb.skillz(&["profile", "remove", "web", "tdd", "-y"]);
    let dry = sb.skillz(&["profile", "remove", "web", "tdd-extra", "--dry-run"]);

    assert!(!ambiguous.status.success() && dry.status.success());
    assert_eq!(sb.profile("web").entries.len(), 2);
}

#[test]
fn the_listing_compares_each_profile_with_the_project() {
    let sb = with_a_loaded_and_b_not();

    let rows = sb.json(&["profile", "list", "--json"]);

    assert_eq!(row(&rows, "a")["matches"], true);
    assert_eq!(row(&rows, "b")["matches"], false);
    assert_eq!(row(&rows, "b")["missing"], 1);
    assert_eq!(row(&rows, "b")["extra"], 1);
}

#[test]
fn status_names_what_is_present_missing_and_extra() {
    let sb = with_a_loaded_and_b_not();

    let rows = sb.json(&["profile", "status", "b", "--json"]);

    let b = row(&rows, "b");
    assert_eq!(b["present"].as_array().unwrap().len(), 0);
    assert_eq!(b["missing"].as_array().unwrap().len(), 1);
    assert_eq!(b["extra"].as_array().unwrap().len(), 1);
}

#[test]
fn status_without_a_name_covers_every_profile() {
    let sb = with_a_loaded_and_b_not();

    let rows = sb.json(&["profile", "status", "--json"]);

    assert_eq!(rows.as_array().unwrap().len(), 2);
    assert_eq!(row(&rows, "a")["matches"], true);
}

#[test]
fn status_of_a_missing_profile_fails() {
    let sb = Sandbox::new();

    let out = sb.skillz(&["profile", "status", "ghost"]);

    assert!(!out.status.success());
}

#[test]
fn a_profile_another_one_extends_cannot_be_deleted() {
    let sb = Sandbox::new();
    sb.write_profile("base", &hook_profile("Stop", "say base", "\"claude\""));
    sb.write_profile("web", "extends = [\"base\"]\n");

    let out = sb.skillz(&["profile", "delete", "base"]);

    assert!(!out.status.success());
    assert_eq!(sb.profile("base").entries.len(), 1);
}
