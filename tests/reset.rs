//! `skillz reset`: wipes skills, and hooks or MCP servers only when asked.

mod common;

use common::*;
use skillz::cli::Filter;

fn global_hooks() -> Filter {
    Filter {
        hooks: true,
        global: true,
        ..Filter::default()
    }
}

fn global_mcp() -> Filter {
    Filter {
        global: true,
        mcp: true,
        ..Filter::default()
    }
}

fn user_mcp_servers(w: &World) -> serde_json::Value {
    read_json(&w.home.join(".claude.json"))["mcpServers"].clone()
}

fn reset_with(w: &World, cmd: skillz::cli::Command, prompter: Scripted) {
    run_quiet(cmd, &deps(w, &Recorder::default(), &prompter)).unwrap();
}

#[test]
fn a_dry_run_removes_nothing() {
    let w = world();

    reset_with(&w, reset(global(), &[], true), Scripted::yes());

    assert!(w.tdd_store().exists());
}

#[test]
fn reset_removes_skills() {
    let w = world();

    reset_with(&w, reset(global(), &[], false), Scripted::yes());

    assert!(!w.tdd_store().exists());
}

#[test]
fn reset_leaves_hooks_unless_asked() {
    let w = world();

    reset_with(&w, reset(global(), &[], false), Scripted::yes());

    assert!(read_json(&w.claude_settings())["hooks"]["PreToolUse"].is_array());
}

#[test]
fn reset_with_the_hooks_filter_removes_hooks() {
    let w = world();

    reset_with(&w, reset(global_hooks(), &[], false), Scripted::yes());

    assert!(
        read_json(&w.claude_settings())["hooks"]
            .get("PreToolUse")
            .is_none()
    );
}

#[test]
fn declining_the_confirmation_removes_nothing() {
    let w = world();

    reset_with(&w, reset(global(), &[], false), Scripted::declining());

    assert!(w.tdd_store().exists());
}

#[test]
fn keep_spares_what_it_names_whatever_the_case() {
    let w = world();

    reset_with(&w, reset(global(), &["TDD"], false), Scripted::yes());

    assert!(w.tdd_store().exists());
}

#[test]
fn reset_leaves_mcp_servers_unless_asked() {
    let w = mcp_world();

    reset_with(&w, reset_forced(global()), Scripted::yes());

    assert!(user_mcp_servers(&w).get("user-fs").is_some());
}

#[test]
fn reset_with_the_mcp_filter_removes_mcp_servers() {
    let w = mcp_world();

    reset_with(&w, reset_forced(global_mcp()), Scripted::yes());

    assert!(user_mcp_servers(&w).get("user-fs").is_none());
}
