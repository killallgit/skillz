//! Hooks as profile entries (`kind = "hook"`), through the binary.

mod common;

use common::*;
use serde_json::json;

const CLAUDE: &str = "\"claude\"";
const CODEX: &str = "\"codex\"";

fn stop_hook(command: &str) -> String {
    hook_profile("Stop", command, CLAUDE)
}

fn handler(command: &str) -> serde_json::Value {
    json!({"hooks": [{"type": "command", "command": command}]})
}

#[test]
fn using_a_profile_adds_its_hook_to_the_project_local_settings() {
    let sb = Sandbox::new();
    sb.write_profile("notify", &stop_hook("say done"));

    let out = sb.skillz(&["use", "-y", "notify"]);

    // Assert: the command is registered under Stop
    assert!(out.status.success());
    assert_eq!(
        hook_commands(&sb.project_claude_hooks(), "Stop"),
        ["say done"]
    );
}

#[test]
fn a_matcher_puts_the_hook_in_a_group_with_that_matcher() {
    let sb = Sandbox::new();
    let entry = "[[entry]]\nkind = \"hook\"\nevent = \"PreToolUse\"\nmatcher = \"Bash\"\ncommand = \"guard\"\nagents = [\"claude\"]\n";
    sb.write_profile("guard", entry);

    sb.skillz(&["use", "-y", "guard"]);

    // Assert: the group's matcher is Bash
    let settings = read_json(&sb.project_claude_hooks());
    assert_eq!(settings["hooks"]["PreToolUse"][0]["matcher"], "Bash");
}

#[test]
fn a_hook_is_added_next_to_existing_hooks_of_the_event() {
    let sb = Sandbox::new();
    write(
        &sb.project_claude_hooks(),
        &json!({"hooks": {"Stop": [handler("mine")]}}).to_string(),
    );
    sb.write_profile("notify", &stop_hook("say done"));

    sb.skillz(&["use", "-y", "notify"]);

    // Assert: both handlers are there
    assert_eq!(
        hook_commands(&sb.project_claude_hooks(), "Stop"),
        ["mine", "say done"]
    );
}

#[test]
fn exact_removes_the_hooks_the_profile_does_not_list_and_keeps_the_rest_of_the_settings() {
    let sb = Sandbox::new();
    write(
        &sb.project_claude_hooks(),
        &json!({"model": "opus", "hooks": {"Stop": [handler("mine")]}}).to_string(),
    );
    sb.write_profile("notify", &stop_hook("say done"));

    let out = sb.skillz(&["use", "-y", "--exact", "notify"]);

    // Assert: only the profile's hook is left, and the unrelated setting is untouched
    assert!(out.status.success());
    assert_eq!(
        hook_commands(&sb.project_claude_hooks(), "Stop"),
        ["say done"]
    );
    assert_eq!(read_json(&sb.project_claude_hooks())["model"], "opus");
}

#[test]
fn using_a_profile_without_exact_never_removes_hooks() {
    let sb = Sandbox::new();
    sb.write_profile("a", &stop_hook("say a"));
    sb.write_profile("b", &stop_hook("say b"));
    sb.skillz(&["use", "-y", "a"]);

    sb.skillz(&["use", "-y", "b"]);

    // Assert: both hooks are there
    assert_eq!(
        hook_commands(&sb.project_claude_hooks(), "Stop"),
        ["say a", "say b"]
    );
}

#[test]
fn exact_switching_replaces_the_old_hook_with_the_new_one() {
    let sb = Sandbox::new();
    sb.write_profile("a", &stop_hook("say a"));
    sb.write_profile("b", &stop_hook("say b"));
    sb.skillz(&["use", "-y", "a"]);

    sb.skillz(&["use", "-y", "--exact", "b"]);

    // Assert: only b's hook remains
    assert_eq!(hook_commands(&sb.project_claude_hooks(), "Stop"), ["say b"]);
}

#[test]
fn switching_to_a_profile_with_the_same_hook_keeps_it() {
    let sb = Sandbox::new();
    sb.write_profile("a", &stop_hook("say same"));
    sb.write_profile("b", &stop_hook("say same"));
    sb.skillz(&["use", "-y", "a"]);

    sb.skillz(&["use", "-y", "b"]);

    // Assert: still exactly one handler
    assert_eq!(
        hook_commands(&sb.project_claude_hooks(), "Stop"),
        ["say same"]
    );
}

#[test]
fn applying_the_same_profile_twice_does_not_duplicate_the_hook() {
    let sb = Sandbox::new();
    sb.write_profile("notify", &stop_hook("say done"));
    sb.skillz(&["use", "-y", "notify"]);

    let out = sb.skillz(&["use", "-y", "notify"]);

    // Assert: one handler, and the second run succeeded
    assert!(out.status.success());
    assert_eq!(
        hook_commands(&sb.project_claude_hooks(), "Stop"),
        ["say done"]
    );
}

#[test]
fn a_hook_already_in_the_settings_is_not_added_again() {
    let sb = Sandbox::new();
    write(
        &sb.project_claude_hooks(),
        &json!({"hooks": {"Stop": [handler("say done")]}}).to_string(),
    );
    sb.write_profile("notify", &stop_hook("say done"));

    sb.skillz(&["use", "-y", "notify"]);

    // Assert: still one handler
    assert_eq!(
        hook_commands(&sb.project_claude_hooks(), "Stop"),
        ["say done"]
    );
}

#[test]
fn a_project_hook_does_not_touch_the_global_settings() {
    let sb = Sandbox::new();
    sb.write_profile("notify", &stop_hook("say done"));

    sb.skillz(&["use", "-y", "notify"]);

    // Assert: nothing in the redirected Claude dir
    assert!(!present(sb.claude.join("settings.json")));
}

#[test]
fn a_codex_hook_goes_to_the_project_hooks_json() {
    let sb = Sandbox::new();
    sb.write_profile("notify", &hook_profile("Stop", "say done", CODEX));

    sb.skillz(&["use", "-y", "notify"]);

    // Assert: registered in .codex/hooks.json, and Claude's settings are untouched
    assert_eq!(
        hook_commands(&sb.project_codex_hooks(), "Stop"),
        ["say done"]
    );
    assert!(!present(sb.project_claude_hooks()));
}

#[test]
fn exact_removes_a_codex_hook_the_profile_lacks_and_keeps_the_listed_ones() {
    let sb = Sandbox::new();
    write(
        &sb.project_codex_hooks(),
        &json!({"hooks": {"Stop": [handler("mine")]}}).to_string(),
    );
    sb.write_profile("notify", &hook_profile("Stop", "say done", CODEX));
    sb.write_profile("keep", &hook_profile("Stop", "mine", CODEX));
    sb.skillz(&["use", "-y", "notify"]);

    sb.skillz(&["use", "-y", "--exact", "keep"]);

    // Assert: only the user's hook remains
    assert_eq!(hook_commands(&sb.project_codex_hooks(), "Stop"), ["mine"]);
}

#[test]
fn a_profile_without_agents_puts_the_hook_in_both_agents() {
    let sb = Sandbox::new();
    sb.write_profile(
        "both",
        "[[entry]]\nkind = \"hook\"\nevent = \"Stop\"\ncommand = \"say done\"\n",
    );

    sb.skillz(&["use", "-y", "both"]);

    // Assert: Claude's and Codex's project files both have it
    assert_eq!(
        hook_commands(&sb.project_claude_hooks(), "Stop"),
        ["say done"]
    );
    assert_eq!(
        hook_commands(&sb.project_codex_hooks(), "Stop"),
        ["say done"]
    );
}

#[test]
fn a_dry_run_writes_no_hook() {
    let sb = Sandbox::new();
    sb.write_profile("notify", &stop_hook("say done"));

    let out = sb.skillz(&["use", "--dry-run", "notify"]);

    // Assert: success, and no settings file
    assert!(out.status.success());
    assert!(!present(sb.project_claude_hooks()));
}

#[test]
fn saving_a_profile_keeps_the_hooks_in_the_config() {
    let sb = Sandbox::new();
    write(
        &sb.project.join(".agents/skills/tdd/SKILL.md"),
        "---\nname: tdd\n---\n",
    );
    write(
        &sb.project.join("skills-lock.json"),
        &json!({"version": 3, "skills": {"tdd": {"source": "mattpocock/skills"}}}).to_string(),
    );
    std::fs::create_dir_all(sb.project.join(".claude/skills")).unwrap();
    std::os::unix::fs::symlink(
        sb.project.join(".agents/skills/tdd"),
        sb.project.join(".claude/skills/tdd"),
    )
    .unwrap();
    write(
        &sb.project_claude_hooks(),
        &json!({"hooks": {"Stop": [handler("say done")]}}).to_string(),
    );

    let out = sb.skillz(&["profile", "save", "mine"]);

    // Assert: the profile holds the skill and the hook
    let saved = skillz::profile::Store::new(sb.skillz_dir().join("store.json"))
        .load("mine")
        .unwrap();
    assert!(out.status.success());
    assert!(
        saved
            .entries
            .iter()
            .any(|e| matches!(e, skillz::profile::Entry::Skill { names, .. } if names == &["tdd"]))
    );
    assert!(saved.entries.iter().any(
        |e| matches!(e, skillz::profile::Entry::Hook { command, .. } if command == "say done")
    ));
}
