//! Profiles that start from other profiles, through the binary.

mod common;

use common::*;

fn skill_entry(source: &str, name: &str) -> String {
    format!("[[entry]]\nkind = \"skill\"\nsource = \"{source}\"\nnames = [\"{name}\"]\n")
}

fn extending(bases: &str, entries: &str) -> String {
    format!("extends = [{bases}]\n{entries}")
}

#[test]
fn a_profile_loads_the_entries_of_the_profile_it_extends() {
    // Assemble
    let sb = Sandbox::new();
    sb.write_profile("base", &skill_entry("o/base", "tdd"));
    sb.write_profile(
        "web",
        &extending("\"base\"", &skill_entry("o/web", "grill")),
    );

    // Act
    let out = sb.skillz(&["use", "-y", "web"]);

    // Assert: npx installed both the parent's and the child's source
    let calls = sb.npx_calls().join("\n");
    assert!(out.status.success() && calls.contains("o/base") && calls.contains("o/web"));
}

#[test]
fn entries_of_a_base_of_a_base_are_loaded_too() {
    // Assemble
    let sb = Sandbox::new();
    sb.write_profile("root", &skill_entry("o/root", "a"));
    sb.write_profile("mid", &extending("\"root\"", &skill_entry("o/mid", "b")));
    sb.write_profile("web", &extending("\"mid\"", ""));

    // Act
    sb.skillz(&["use", "-y", "web"]);

    // Assert: the grandparent's source was installed
    assert!(sb.npx_calls().join("\n").contains("o/root"));
}

#[test]
fn hooks_of_the_parent_and_the_child_both_land_in_the_settings() {
    // Assemble
    let sb = Sandbox::new();
    sb.write_profile("base", &hook_profile("Stop", "say base", "\"claude\""));
    sb.write_profile(
        "web",
        &extending("\"base\"", &hook_profile("Stop", "say web", "\"claude\"")),
    );

    // Act
    sb.skillz(&["use", "-y", "web"]);

    // Assert: both commands are registered under Stop
    let mut commands = hook_commands(&sb.project_claude_hooks(), "Stop");
    commands.sort();
    assert_eq!(commands, ["say base", "say web"]);
}

#[test]
fn a_child_repeating_a_parent_hook_registers_it_once() {
    // Assemble
    let sb = Sandbox::new();
    let same = hook_profile("Stop", "say hi", "\"claude\"");
    sb.write_profile("base", &same);
    sb.write_profile("web", &extending("\"base\"", &same));

    // Act
    sb.skillz(&["use", "-y", "web"]);

    // Assert: a single handler
    assert_eq!(
        hook_commands(&sb.project_claude_hooks(), "Stop"),
        ["say hi"]
    );
}

#[test]
fn a_cycle_between_profiles_is_rejected() {
    // Assemble
    let sb = Sandbox::new();
    sb.write_profile("a", &extending("\"b\"", &skill_entry("o/a", "x")));
    sb.write_profile("b", &extending("\"a\"", &skill_entry("o/b", "y")));

    // Act
    let out = sb.skillz(&["use", "-y", "a"]);

    // Assert: a non-zero exit
    assert!(!out.status.success());
}

#[test]
fn a_rejected_cycle_installs_nothing() {
    // Assemble
    let sb = Sandbox::new();
    sb.write_profile("a", &extending("\"b\"", &skill_entry("o/a", "x")));
    sb.write_profile("b", &extending("\"a\"", &skill_entry("o/b", "y")));

    // Act
    sb.skillz(&["use", "-y", "a"]);

    // Assert: npx never ran
    assert!(sb.npx_calls().is_empty());
}

#[test]
fn a_profile_extending_itself_is_rejected() {
    // Assemble
    let sb = Sandbox::new();
    sb.write_profile("a", &extending("\"a\"", &skill_entry("o/a", "x")));

    // Act
    let out = sb.skillz(&["use", "-y", "a"]);

    // Assert: a non-zero exit
    assert!(!out.status.success());
}

#[test]
fn extending_a_missing_profile_is_rejected() {
    // Assemble
    let sb = Sandbox::new();
    sb.write_profile("web", &extending("\"ghost\"", &skill_entry("o/web", "x")));

    // Act
    let out = sb.skillz(&["use", "-y", "web"]);

    // Assert: a non-zero exit
    assert!(!out.status.success());
}

fn skills_entry(source: &str, names: &[&str]) -> String {
    let list: Vec<String> = names.iter().map(|n| format!("\"{n}\"")).collect();
    format!(
        "[[entry]]\nkind = \"skill\"\nsource = \"{source}\"\nnames = [{}]\n",
        list.join(", ")
    )
}

#[test]
fn a_child_skill_overrides_the_same_skill_of_the_parent() {
    // Assemble
    let sb = Sandbox::new();
    sb.write_profile("base", &skills_entry("o/s", &["a", "b"]));
    sb.write_profile("web", &extending("\"base\"", &skills_entry("o/s", &["a"])));

    // Act
    sb.skillz(&["use", "-y", "web"]);

    // Assert: skill a is installed by one call only, b by the parent's
    let calls = sb.npx_calls();
    let installing = |skill: &str| {
        calls
            .iter()
            .filter(|c| c.split_whitespace().any(|w| w == skill))
            .count()
    };
    assert_eq!((installing("a"), installing("b")), (1, 1));
}

#[test]
fn a_child_hook_with_other_agents_takes_the_hook_from_the_parents_agents() {
    // Assemble
    let sb = Sandbox::new();
    sb.write_profile("base", &hook_profile("Stop", "say hi", "\"claude\""));
    sb.write_profile(
        "web",
        &extending("\"base\"", &hook_profile("Stop", "say hi", "\"codex\"")),
    );

    // Act
    sb.skillz(&["use", "-y", "web"]);

    // Assert: only the child's agent has the hook
    assert_eq!(
        (
            hook_commands(&sb.project_claude_hooks(), "Stop"),
            hook_commands(&sb.project_codex_hooks(), "Stop")
        ),
        (vec![], vec!["say hi".to_string()])
    );
}
