//! `skillz sync`: the library gains what is installed and never loses anything.

mod common;

use std::fs;

use common::*;
use serde_json::Value;
use skillz::app::library::Catalog;
use skillz::app::store::Store;
use skillz::cli::{Command, SyncArgs};

fn sync_command(dry_run: bool) -> Command {
    Command::Sync(SyncArgs {
        project: None,
        global: false,
        dry_run,
        json: true,
    })
}

fn sync_json(w: &World, dry_run: bool) -> Value {
    let (runner, prompter) = (Recorder::default(), Scripted::yes());
    let out = run_captured(sync_command(dry_run), &deps(w, &runner, &prompter));
    serde_json::from_slice(&out).unwrap()
}

fn library(w: &World) -> Catalog {
    Store::new(w.skillz_dir().join("store.json"))
        .read()
        .unwrap()
        .items
}

fn count(report: &Value, key: &str) -> usize {
    report[key].as_array().unwrap().len()
}

#[test]
fn sync_adds_installed_items_from_both_scopes_and_reports_what_needs_secrets() {
    let w = mcp_world();

    let report = sync_json(&w, false);

    // Assert the skill, hooks and servers are stored and only the keyed server is left out.
    assert_eq!(count(&report, "added"), library(&w).items().len());
    assert!(count(&report, "added") > 3);
    assert_eq!(count(&report, "skipped"), 1);
    assert!(
        library(&w)
            .items()
            .iter()
            .all(|item| { item.provenance.as_ref().is_some_and(|p| p.detected) })
    );
}

#[test]
fn a_second_sync_adds_nothing() {
    let w = mcp_world();
    sync_json(&w, false);
    let first = library(&w);

    let again = sync_json(&w, false);

    assert_eq!((count(&again, "added"), library(&w)), (0, first));
}

#[test]
fn a_dry_run_reports_without_storing() {
    let w = mcp_world();

    let report = sync_json(&w, true);

    assert!(count(&report, "added") > 0);
    assert!(library(&w).items().is_empty());
}

#[test]
fn items_removed_from_disk_stay_in_the_library() {
    let w = mcp_world();
    sync_json(&w, false);
    let known = library(&w).items().len();
    fs::remove_file(w.claude_settings()).unwrap();
    fs::remove_dir_all(w.home.join(".agents")).unwrap();

    let again = sync_json(&w, false);

    assert_eq!(
        (count(&again, "added"), library(&w).items().len()),
        (0, known)
    );
}
