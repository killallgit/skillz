//! `skillz prune`: duplicate loose skills and unused plugins.

mod common;

use std::path::PathBuf;

use common::*;
use serde_json::json;
use skillz::cli::{Command, Filter, PruneArgs};

const LINT_SKILL: &str = "---\nname: lint\n---\n";

/// A loose skill `lint` that duplicates the one shipped by the never-used plugin `tools@mp`.
fn world_with_duplicate_and_unused_plugin() -> (World, PathBuf) {
    let w = world();
    let cache = w.home.join(".claude/plugins/cache/mp/tools/1.0");
    write(&cache.join("skills/lint/SKILL.md"), LINT_SKILL);
    write(
        &w.home.join(".claude/plugins/installed_plugins.json"),
        &json!({"version": 2, "plugins": {"tools@mp": [
            {"scope": "user", "installPath": cache.to_string_lossy()}
        ]}})
        .to_string(),
    );
    write(&w.home.join(".claude/skills/lint/SKILL.md"), LINT_SKILL);
    (w, cache)
}

fn prune(dry_run: bool, force: bool) -> Command {
    Command::Prune(PruneArgs {
        project: None,
        filter: Filter::default(),
        dry_run,
        force,
    })
}

/// `main` swaps in a prompter that picks and confirms everything for `--force`; so does this.
fn pruned(w: &World, cmd: Command, prompter: Scripted) {
    run_quiet(cmd, &deps(w, &Recorder::default(), &prompter)).unwrap();
}

#[test]
fn a_dry_run_keeps_the_duplicate() {
    let (w, _cache) = world_with_duplicate_and_unused_plugin();

    pruned(&w, prune(true, false), Scripted::picking(vec![]));

    assert!(w.home.join(".claude/skills/lint").exists());
}

#[test]
fn prune_removes_the_duplicate_loose_skill() {
    let (w, _cache) = world_with_duplicate_and_unused_plugin();

    pruned(&w, prune(false, false), Scripted::picking(vec![]));

    assert!(!w.home.join(".claude/skills/lint").exists());
}

#[test]
fn prune_keeps_unused_items_that_were_not_picked() {
    let (w, cache) = world_with_duplicate_and_unused_plugin();

    pruned(&w, prune(false, false), Scripted::picking(vec![]));

    assert!(cache.exists());
}

#[test]
fn prune_leaves_items_that_are_not_candidates() {
    let (w, _cache) = world_with_duplicate_and_unused_plugin();

    pruned(&w, prune(false, false), Scripted::picking(vec![]));

    assert!(w.tdd_store().exists());
}

#[test]
fn force_removes_the_duplicate() {
    let (w, _cache) = world_with_duplicate_and_unused_plugin();

    pruned(&w, prune(false, true), Scripted::yes());

    assert!(!w.home.join(".claude/skills/lint").exists());
}

#[test]
fn force_removes_every_unused_item() {
    let (w, cache) = world_with_duplicate_and_unused_plugin();

    pruned(&w, prune(false, true), Scripted::yes());

    assert!(!w.tdd_store().exists());
    assert!(!cache.exists());
}
