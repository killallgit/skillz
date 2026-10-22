//! `skillz library ...`: the Library tab's features from the command line, through the binary.

mod common;

use common::*;
use skillz::profile::Entry;

const SAY_HI: &str = "hook Stop say hi";

fn with_library(lines: &[&str]) -> Sandbox {
    let sb = Sandbox::new();
    for line in lines {
        assert!(sb.skillz(&["library", "add", line]).status.success());
    }
    sb
}

fn kinds(rows: &serde_json::Value) -> Vec<String> {
    let kind = |row: &serde_json::Value| row["kind"].as_str().unwrap().to_string();
    rows.as_array().unwrap().iter().map(kind).collect()
}

#[test]
fn a_typed_line_is_added_to_the_library_without_provenance() {
    // Assemble
    let sb = Sandbox::new();

    // Act
    let out = sb.skillz(&["library", "add", "skill o/r tdd"]);

    // Assert
    let library = sb.library();
    assert!(out.status.success());
    assert_eq!(library.items().len(), 1);
    assert!(library.items()[0].provenance.is_none());
}

#[test]
fn a_line_with_a_secret_or_bad_syntax_fails_and_adds_nothing() {
    // Assemble
    let sb = Sandbox::new();

    // Act
    let secret = sb.skillz(&["library", "add", "hook Stop run --token abc"]);
    let malformed = sb.skillz(&["library", "add", "skill o/r"]);

    // Assert
    assert!(!secret.status.success() && !malformed.status.success());
    assert!(sb.library().items().is_empty());
}

#[test]
fn a_dry_run_add_checks_the_line_and_stores_nothing() {
    // Assemble
    let sb = Sandbox::new();

    // Act
    let fine = sb.skillz(&["library", "add", "--dry-run", SAY_HI]);
    let bad = sb.skillz(&["library", "add", "--dry-run", "nonsense"]);

    // Assert
    assert!(fine.status.success() && !bad.status.success());
    assert!(sb.library().items().is_empty());
}

#[test]
fn the_listing_says_where_an_item_came_from_and_where_it_is_installed() {
    // Assemble
    let sb = with_library(&[SAY_HI, "skill o/r tdd"]);
    assert!(
        sb.skillz(&["library", "install", "say hi", "-y"])
            .status
            .success()
    );

    // Act
    let rows = sb.json(&["library", "list", "--json"]);

    // Assert
    let row = |kind: &str| {
        let rows = rows.as_array().unwrap();
        rows.iter().find(|r| r["kind"] == kind).unwrap().clone()
    };
    assert_eq!(row("hook")["installed"]["project"], true);
    assert_eq!(row("skill")["installed"]["project"], false);
    assert_eq!(row("skill")["origin"], "manual");
    assert_eq!(row("skill")["source"], "o/r");
}

#[test]
fn available_lists_what_the_library_offers_and_the_project_lacks() {
    // Assemble
    let sb = with_library(&[SAY_HI, "skill o/r tdd"]);
    sb.skillz(&["library", "install", "say hi", "-y"]);

    // Act
    let in_project = sb.json(&["list", "--available", "--json"]);
    let for_user = sb.json(&["list", "--available", "--json", "-g"]);

    // Assert: the hook is installed in the project but not for the user
    assert_eq!(kinds(&in_project), ["skill"]);
    assert_eq!(kinds(&for_user).len(), 2);
}

#[test]
fn available_cannot_be_combined_with_a_check_for_updates() {
    // Assemble
    let sb = Sandbox::new();

    // Act
    let out = sb.skillz(&["list", "--available", "--json", "--check"]);

    // Assert
    assert!(!out.status.success());
}

#[test]
fn the_installed_listing_gives_each_items_location_and_optionally_its_update_marker() {
    // Assemble
    let sb = with_library(&[SAY_HI]);
    sb.skillz(&["library", "install", "say hi", "-y"]);

    // Act
    let plain = sb.json(&["list", "--json"]);
    let checked = sb.json(&["list", "--json", "--check"]);

    // Assert
    let first = |rows: &serde_json::Value| rows.as_array().unwrap()[0].clone();
    assert!(first(&plain)["location"].is_string());
    assert!(first(&plain).get("update_available").is_none());
    assert_eq!(first(&checked)["update_available"], false);
}

#[test]
fn a_check_without_json_is_refused() {
    // Assemble
    let sb = Sandbox::new();

    // Act
    let out = sb.skillz(&["list", "--check"]);

    // Assert
    assert!(!out.status.success());
}

#[test]
fn the_listing_can_be_limited_to_kinds() {
    // Assemble
    let sb = with_library(&[SAY_HI, "skill o/r tdd"]);

    // Act
    let rows = sb.json(&["library", "list", "--json", "--skills"]);

    // Assert
    assert_eq!(kinds(&rows), ["skill"]);
}

#[test]
fn installing_a_hook_item_registers_it_in_the_project() {
    // Assemble
    let sb = with_library(&[SAY_HI]);

    // Act
    let out = sb.skillz(&["library", "install", "say hi", "-y"]);

    // Assert
    assert!(out.status.success());
    assert_eq!(
        hook_commands(&sb.project_claude_hooks(), "Stop"),
        ["say hi"]
    );
}

#[test]
fn installing_several_items_runs_one_plan_for_all_of_them() {
    // Assemble
    let sb = with_library(&["skill o/r tdd", "skill o/other grill"]);

    // Act
    let out = sb.skillz(&["library", "install", "tdd", "grill", "-y"]);

    // Assert
    let calls = sb.npx_calls().join("\n");
    assert!(out.status.success() && calls.contains("o/r") && calls.contains("o/other"));
}

#[test]
fn a_dry_run_install_changes_nothing() {
    // Assemble
    let sb = with_library(&[SAY_HI, "skill o/r tdd"]);

    // Act
    let out = sb.skillz(&["library", "install", "say hi", "tdd", "--dry-run"]);

    // Assert
    assert!(out.status.success());
    assert!(!sb.project_claude_hooks().exists() && sb.npx_calls().is_empty());
}

#[test]
fn installing_globally_writes_to_the_user_config() {
    // Assemble
    let sb = with_library(&[SAY_HI]);

    // Act
    let out = sb.skillz(&["library", "install", "-g", "say hi", "-y"]);

    // Assert
    assert!(out.status.success());
    assert!(!sb.project_claude_hooks().exists());
    assert_eq!(
        hook_commands(&sb.claude.join("settings.json"), "Stop"),
        ["say hi"]
    );
}

#[test]
fn installing_what_the_library_lacks_fails() {
    // Assemble
    let sb = with_library(&[SAY_HI]);

    // Act
    let out = sb.skillz(&["library", "install", "nothing like it", "-y"]);

    // Assert
    assert!(!out.status.success());
}

#[test]
fn removing_by_query_takes_the_item_out_of_the_library() {
    // Assemble
    let sb = with_library(&[SAY_HI, "skill o/r tdd"]);

    // Act
    let out = sb.skillz(&["library", "remove", "tdd", "-y"]);

    // Assert
    let left = sb.library();
    assert!(out.status.success());
    assert!(matches!(left.items(), [item] if matches!(item.entry, Entry::Hook { .. })));
}

#[test]
fn removing_by_the_line_that_added_the_item_works_too() {
    // Assemble
    let sb = with_library(&[SAY_HI]);

    // Act
    let out = sb.skillz(&["library", "remove", "-y", SAY_HI]);

    // Assert
    assert!(out.status.success() && sb.library().items().is_empty());
}

#[test]
fn an_ambiguous_query_is_refused_with_yes_and_a_dry_run_removes_nothing() {
    // Assemble
    let sb = with_library(&["skill o/r tdd", "skill o/r tdd-extra"]);

    // Act
    let ambiguous = sb.skillz(&["library", "remove", "tdd", "-y"]);
    let dry = sb.skillz(&["library", "remove", "tdd-extra", "--dry-run"]);

    // Assert
    assert!(!ambiguous.status.success() && dry.status.success());
    assert_eq!(sb.library().items().len(), 2);
}
