//! `install -g` and `update -g` through the binary: the user's directories change, the project's
//! do not, and the other way round.

mod common;

use common::*;

const NPX_INSTALLING_TDD: &str = r#"echo "$@" >> "$HOME/npx.log"
case " $* " in *" add "*) ;; *) exit 0;; esac
case " $* " in *" -g "*) base="$HOME";; *) base="$PWD";; esac
mkdir -p "$base/.agents/skills/tdd"
printf -- '---\nname: tdd\ndescription: d\n---\n' > "$base/.agents/skills/tdd/SKILL.md"
"#;

fn sandbox_with_source() -> Sandbox {
    let sb = Sandbox::new();
    sb.fake_program("npx", NPX_INSTALLING_TDD);
    write(
        &sb.home.join("src/skills/tdd/SKILL.md"),
        "---\nname: tdd\ndescription: d\n---\n",
    );
    sb
}

fn install(sb: &Sandbox, scope_flags: &[&str]) -> std::process::Output {
    let source = sb.home.join("src").display().to_string();
    let mut args = vec!["install", &source, "--all", "-a", "codex", "-y"];
    args.extend(scope_flags);
    sb.skillz(&args)
}

#[test]
fn install_g_changes_only_the_global_directories() {
    // Assemble
    let sb = sandbox_with_source();

    // Act
    let out = install(&sb, &["-g"]);

    // Assert
    assert!(out.status.success());
    assert!(present(sb.home.join(".agents/skills/tdd")));
    assert!(!present(sb.project.join(".agents/skills/tdd")));
    assert!(sb.npx_calls()[0].contains(" -g"));
}

#[test]
fn install_without_g_leaves_the_global_directories_alone() {
    // Assemble
    let sb = sandbox_with_source();

    // Act
    let out = install(&sb, &[]);

    // Assert
    assert!(out.status.success());
    assert!(present(sb.project.join(".agents/skills/tdd")));
    assert!(!present(sb.home.join(".agents/skills/tdd")));
    assert!(!sb.npx_calls()[0].contains(" -g"));
}

#[test]
fn install_g_and_a_project_directory_are_exclusive() {
    // Assemble
    let sb = sandbox_with_source();

    // Act
    let out = install(&sb, &["-g", "-p", "."]);

    // Assert
    assert!(!out.status.success());
    assert!(sb.npx_calls().is_empty());
}

#[test]
fn update_g_updates_the_global_skills_with_the_global_flag() {
    // Assemble
    let sb = sandbox_with_source();
    install(&sb, &["-g"]);
    write(
        &sb.home.join(".agents/.skill-lock.json"),
        r#"{"version":3,"skills":{"tdd":{"source":"o/r","sourceType":"github"}}}"#,
    );

    // Act
    let out = sb.skillz(&["update", "tdd", "-g", "-y"]);

    // Assert
    let update = sb.npx_calls().into_iter().find(|c| c.contains("update"));
    assert!(out.status.success());
    assert!(update.is_some_and(|call| call.contains(" -g")));
}
