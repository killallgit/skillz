//! `skillz run PROFILE -- COMMAND` through the binary.

mod common;

use common::*;

const HOOK: &str = "say isolated";

fn sandbox_with_notify() -> Sandbox {
    let sb = Sandbox::new();
    sb.write_profile("notify", &hook_profile("Stop", HOOK, "\"claude\""));
    sb
}

fn envs_dir(sb: &Sandbox, profile: &str) -> std::path::PathBuf {
    sb.skillz_dir().join("envs").join(profile)
}

/// Run `printenv VAR` through skillz and return what the child saw.
fn child_env(sb: &Sandbox, var: &str) -> String {
    let dump = sb.home.join("seen.txt");
    let script = format!("printenv {var} > \"$1\"");
    sb.skillz(&[
        "run",
        "-y",
        "notify",
        "--",
        "sh",
        "-c",
        &script,
        "_",
        dump.to_str().unwrap(),
    ]);
    std::fs::read_to_string(dump).unwrap().trim().to_string()
}

#[test]
fn the_command_sees_claude_config_dir_under_the_skillz_envs_dir() {
    let sb = sandbox_with_notify();

    let seen = child_env(&sb, "CLAUDE_CONFIG_DIR");

    // Assert: the profile's own claude dir, not the sandbox's
    assert_eq!(
        seen,
        envs_dir(&sb, "notify").join("claude").to_str().unwrap()
    );
}

#[test]
fn the_command_sees_codex_home_under_the_skillz_envs_dir() {
    let sb = sandbox_with_notify();

    let seen = child_env(&sb, "CODEX_HOME");

    // Assert: the profile's own codex dir
    assert_eq!(
        seen,
        envs_dir(&sb, "notify").join("codex").to_str().unwrap()
    );
}

#[test]
fn the_profile_is_loaded_into_the_isolated_claude_dir() {
    let sb = sandbox_with_notify();

    sb.skillz(&["run", "-y", "notify", "--", "true"]);

    // Assert: the hook is in the isolated settings
    let settings = envs_dir(&sb, "notify").join("claude/settings.json");
    assert_eq!(hook_commands(&settings, "Stop"), [HOOK]);
}

#[test]
fn running_leaves_the_real_config_dirs_untouched() {
    let sb = sandbox_with_notify();

    sb.skillz(&["run", "-y", "notify", "--", "true"]);

    // Assert: neither the redirected dirs nor ~/.claude, ~/.codex were created
    let dirs = [
        sb.claude.clone(),
        sb.codex.clone(),
        sb.home.join(".claude"),
        sb.home.join(".codex"),
    ];
    assert!(!dirs.iter().any(present));
}

#[test]
fn running_leaves_the_project_untouched() {
    let sb = sandbox_with_notify();

    sb.skillz(&["run", "-y", "notify", "--", "true"]);

    // Assert: no project config was written
    assert!(!present(sb.project.join(".claude")) && !present(sb.project.join(".codex")));
}

#[test]
fn existing_real_config_is_left_as_it_was() {
    let sb = sandbox_with_notify();
    let mine = sb.claude.join("settings.json");
    write(&mine, "{\"model\":\"opus\"}");

    sb.skillz(&["run", "-y", "notify", "--", "true"]);

    // Assert: the file is byte for byte as written
    assert_eq!(
        std::fs::read_to_string(mine).unwrap(),
        "{\"model\":\"opus\"}"
    );
}

#[test]
fn a_successful_command_exits_zero() {
    let sb = sandbox_with_notify();

    let out = sb.skillz(&["run", "-y", "notify", "--", "true"]);

    // Assert: success
    assert!(out.status.success());
}

#[test]
fn skillz_exits_with_the_exit_code_of_the_command() {
    let sb = sandbox_with_notify();

    let out = sb.skillz(&["run", "-y", "notify", "--", "sh", "-c", "exit 3"]);

    // Assert: the child's own code, not a generic failure
    assert_eq!(out.status.code(), Some(3));
}

#[test]
fn a_missing_profile_runs_nothing() {
    let sb = Sandbox::new();
    let marker = sb.home.join("ran");

    let out = sb.skillz(&[
        "run",
        "-y",
        "ghost",
        "--",
        "touch",
        marker.to_str().unwrap(),
    ]);

    // Assert: failure and the command never started
    assert!(!out.status.success() && !present(marker));
}

#[test]
fn two_profiles_get_separate_config_dirs() {
    let sb = sandbox_with_notify();
    sb.write_profile("other", &hook_profile("Stop", "say other", "\"claude\""));

    sb.skillz(&["run", "-y", "notify", "--", "true"]);
    sb.skillz(&["run", "-y", "other", "--", "true"]);

    // Assert: each dir holds only its own hook
    let settings = |p: &str| envs_dir(&sb, p).join("claude/settings.json");
    assert_eq!(hook_commands(&settings("notify"), "Stop"), [HOOK]);
    assert_eq!(hook_commands(&settings("other"), "Stop"), ["say other"]);
}
