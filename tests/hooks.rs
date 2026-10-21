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
    // Assemble
    let sb = Sandbox::new();
    sb.write_profile("notify", &stop_hook("say done"));

    // Act
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
    // Assemble
    let sb = Sandbox::new();
    let entry = "[[entry]]\nkind = \"hook\"\nevent = \"PreToolUse\"\nmatcher = \"Bash\"\ncommand = \"guard\"\nagents = [\"claude\"]\n";
    sb.write_profile("guard", entry);

    // Act
    sb.skillz(&["use", "-y", "guard"]);

    // Assert: the group's matcher is Bash
    let settings = read_json(&sb.project_claude_hooks());
    assert_eq!(settings["hooks"]["PreToolUse"][0]["matcher"], "Bash");
}

#[test]
fn a_hook_is_added_next_to_existing_hooks_of_the_event() {
    // Assemble
    let sb = Sandbox::new();
    write(
        &sb.project_claude_hooks(),
        &json!({"hooks": {"Stop": [handler("mine")]}}).to_string(),
    );
    sb.write_profile("notify", &stop_hook("say done"));

    // Act
    sb.skillz(&["use", "-y", "notify"]);

    // Assert: both handlers are there
    assert_eq!(
        hook_commands(&sb.project_claude_hooks(), "Stop"),
        ["mine", "say done"]
    );
}

#[test]
fn unloading_removes_exactly_the_profiles_hook() {
    // Assemble
    let sb = Sandbox::new();
    let before = json!({
        "model": "opus",
        "hooks": {"Stop": [handler("mine")], "SessionStart": [handler("keep me")]}
    });
    write(&sb.project_claude_hooks(), &before.to_string());
    sb.write_profile("notify", &stop_hook("say done"));
    sb.skillz(&["use", "-y", "notify"]);

    // Act
    let out = sb.skillz(&["use", "-y", "--none"]);

    // Assert: the other hooks and settings are as they were
    assert!(out.status.success());
    assert_eq!(read_json(&sb.project_claude_hooks()), before);
}

#[test]
fn switching_away_removes_the_old_hook_and_adds_the_new_one() {
    // Assemble
    let sb = Sandbox::new();
    sb.write_profile("a", &stop_hook("say a"));
    sb.write_profile("b", &stop_hook("say b"));
    sb.skillz(&["use", "-y", "a"]);

    // Act
    sb.skillz(&["use", "-y", "b"]);

    // Assert: only b's hook remains
    assert_eq!(hook_commands(&sb.project_claude_hooks(), "Stop"), ["say b"]);
}

#[test]
fn switching_to_a_profile_with_the_same_hook_keeps_it() {
    // Assemble
    let sb = Sandbox::new();
    sb.write_profile("a", &stop_hook("say same"));
    sb.write_profile("b", &stop_hook("say same"));
    sb.skillz(&["use", "-y", "a"]);

    // Act
    sb.skillz(&["use", "-y", "b"]);

    // Assert: still exactly one handler
    assert_eq!(
        hook_commands(&sb.project_claude_hooks(), "Stop"),
        ["say same"]
    );
}

#[test]
fn applying_the_same_profile_twice_does_not_duplicate_the_hook() {
    // Assemble
    let sb = Sandbox::new();
    sb.write_profile("notify", &stop_hook("say done"));
    sb.skillz(&["use", "-y", "notify"]);

    // Act
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
    // Assemble
    let sb = Sandbox::new();
    write(
        &sb.project_claude_hooks(),
        &json!({"hooks": {"Stop": [handler("say done")]}}).to_string(),
    );
    sb.write_profile("notify", &stop_hook("say done"));

    // Act
    sb.skillz(&["use", "-y", "notify"]);

    // Assert: still one handler
    assert_eq!(
        hook_commands(&sb.project_claude_hooks(), "Stop"),
        ["say done"]
    );
}

#[test]
fn a_project_hook_does_not_touch_the_global_settings() {
    // Assemble
    let sb = Sandbox::new();
    sb.write_profile("notify", &stop_hook("say done"));

    // Act
    sb.skillz(&["use", "-y", "notify"]);

    // Assert: nothing in the redirected Claude dir
    assert!(!present(sb.claude.join("settings.json")));
}

#[test]
fn global_scope_writes_to_the_claude_config_dir_settings() {
    // Assemble
    let sb = Sandbox::new();
    sb.write_profile("notify", &stop_hook("say done"));

    // Act
    let out = sb.skillz(&["use", "-y", "-g", "notify"]);

    // Assert: the hook is in $CLAUDE_CONFIG_DIR/settings.json and not in the project
    assert!(out.status.success());
    assert_eq!(
        hook_commands(&sb.claude.join("settings.json"), "Stop"),
        ["say done"]
    );
    assert!(!present(sb.project_claude_hooks()));
}

#[test]
fn unloading_a_global_profile_removes_only_its_hook() {
    // Assemble
    let sb = Sandbox::new();
    let before = json!({"model": "opus", "hooks": {"Stop": [handler("mine")]}});
    write(&sb.claude.join("settings.json"), &before.to_string());
    sb.write_profile("notify", &stop_hook("say done"));
    sb.skillz(&["use", "-y", "-g", "notify"]);

    // Act
    sb.skillz(&["use", "-y", "-g", "--none"]);

    // Assert: the user's own hook and model remain
    assert_eq!(read_json(&sb.claude.join("settings.json")), before);
}

#[test]
fn a_codex_hook_goes_to_the_project_hooks_json() {
    // Assemble
    let sb = Sandbox::new();
    sb.write_profile("notify", &hook_profile("Stop", "say done", CODEX));

    // Act
    sb.skillz(&["use", "-y", "notify"]);

    // Assert: registered in .codex/hooks.json, and Claude's settings are untouched
    assert_eq!(
        hook_commands(&sb.project_codex_hooks(), "Stop"),
        ["say done"]
    );
    assert!(!present(sb.project_claude_hooks()));
}

#[test]
fn unloading_a_codex_hook_removes_it_and_keeps_the_others() {
    // Assemble
    let sb = Sandbox::new();
    write(
        &sb.project_codex_hooks(),
        &json!({"hooks": {"Stop": [handler("mine")]}}).to_string(),
    );
    sb.write_profile("notify", &hook_profile("Stop", "say done", CODEX));
    sb.skillz(&["use", "-y", "notify"]);

    // Act
    sb.skillz(&["use", "-y", "--none"]);

    // Assert: only the user's hook remains
    assert_eq!(hook_commands(&sb.project_codex_hooks(), "Stop"), ["mine"]);
}

#[test]
fn a_profile_without_agents_puts_the_hook_in_both_agents() {
    // Assemble
    let sb = Sandbox::new();
    sb.write_profile(
        "both",
        "[[entry]]\nkind = \"hook\"\nevent = \"Stop\"\ncommand = \"say done\"\n",
    );

    // Act
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
    // Assemble
    let sb = Sandbox::new();
    sb.write_profile("notify", &stop_hook("say done"));

    // Act
    let out = sb.skillz(&["use", "--dry-run", "notify"]);

    // Assert: success, and no settings file
    assert!(out.status.success());
    assert!(!present(sb.project_claude_hooks()));
}

#[test]
fn saving_a_profile_skips_the_hooks_in_the_config() {
    // Assemble
    let sb = Sandbox::new();
    write(
        &sb.home.join(".agents/skills/tdd/SKILL.md"),
        "---\nname: tdd\n---\n",
    );
    write(
        &sb.home.join(".agents/.skill-lock.json"),
        &json!({"version": 3, "skills": {"tdd": {"source": "mattpocock/skills"}}}).to_string(),
    );
    std::fs::create_dir_all(sb.claude.join("skills")).unwrap();
    std::os::unix::fs::symlink(
        sb.home.join(".agents/skills/tdd"),
        sb.claude.join("skills/tdd"),
    )
    .unwrap();
    write(
        &sb.claude.join("settings.json"),
        &json!({"hooks": {"Stop": [handler("say done")]}}).to_string(),
    );

    // Act
    let out = sb.skillz(&["profile", "save", "-g", "mine"]);

    // Assert: the profile holds the skill and no hook
    let saved = skillz::profile::Store::new(sb.skillz_dir().join("profiles"))
        .load("mine")
        .unwrap();
    assert!(out.status.success());
    assert!(matches!(
        saved.entries.as_slice(),
        [skillz::profile::Entry::Skill { names, .. }] if names == &["tdd"]
    ));
}
