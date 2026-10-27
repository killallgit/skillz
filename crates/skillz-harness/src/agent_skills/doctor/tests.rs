use std::fs;
use std::path::Path;

use super::*;

fn skill_text(name: &str) -> String {
    format!("---\nname: {name}\ndescription: does {name}\n---\nbody\n")
}

fn write(path: impl AsRef<Path>, text: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn skill(root: &Path, dir: &str, name: &str) {
    write(root.join(dir).join("SKILL.md"), &skill_text(name));
}

fn root(dir: &Path, scope: Scope, agents: &[Agent]) -> Root {
    Root {
        dir: dir.to_path_buf(),
        scope,
        agents: agents.to_vec(),
    }
}

fn collisions(found: &[Diagnostic]) -> Vec<(&str, &Readers, usize)> {
    found
        .iter()
        .filter_map(|d| match d {
            Diagnostic::Collision {
                name,
                readers,
                copies,
            } => Some((name.as_str(), readers, copies.len())),
            _ => None,
        })
        .collect()
}

#[test]
fn two_skills_with_one_name_in_folders_one_tool_reads_collide_for_that_tool() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a"), dir.path().join("b"));
    skill(&a, "tdd", "tdd");
    skill(&b, "tdd", "tdd");

    let found = diagnose(&[
        root(&a, Scope::Global, &[Agent::Codex]),
        root(&b, Scope::Project, &[Agent::Codex]),
    ]);

    assert_eq!(
        collisions(&found),
        [("tdd", &Readers::Agents(vec![Agent::Codex]), 2)]
    );
}

#[test]
fn a_name_in_folders_no_single_tool_reads_together_is_not_a_collision() {
    let dir = tempfile::tempdir().unwrap();
    let (claude, codex) = (dir.path().join("claude"), dir.path().join("codex"));
    skill(&claude, "tdd", "tdd");
    skill(&codex, "tdd", "tdd");

    let found = diagnose(&[
        root(&claude, Scope::Global, &[Agent::Claude]),
        root(&codex, Scope::Global, &[Agent::Codex]),
    ]);

    assert_eq!(collisions(&found), []);
}

#[test]
fn a_collision_both_tools_meet_names_both_tools_once() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a"), dir.path().join("b"));
    skill(&a, "tdd", "tdd");
    skill(&b, "tdd", "tdd");
    let both = [Agent::Claude, Agent::Codex];

    let found = diagnose(&[
        root(&a, Scope::Global, &both),
        root(&b, Scope::Project, &both),
    ]);

    assert_eq!(
        collisions(&found),
        [("tdd", &Readers::Agents(both.to_vec()), 2)]
    );
}

#[cfg(unix)]
#[test]
fn a_link_to_the_same_skill_is_one_skill_not_a_collision() {
    let dir = tempfile::tempdir().unwrap();
    let (store, linked) = (dir.path().join("store"), dir.path().join("linked"));
    skill(&store, "tdd", "tdd");
    fs::create_dir_all(&linked).unwrap();
    std::os::unix::fs::symlink(store.join("tdd"), linked.join("tdd")).unwrap();

    let found = diagnose(&[
        root(&store, Scope::Global, &[Agent::Codex]),
        root(&linked, Scope::Global, &[Agent::Codex]),
    ]);

    assert_eq!(found, []);
}

#[cfg(unix)]
#[test]
fn a_link_whose_target_is_gone_is_dangling_and_a_plain_file_is_not() {
    let dir = tempfile::tempdir().unwrap();
    let skills = dir.path().join("skills");
    fs::create_dir_all(&skills).unwrap();
    std::os::unix::fs::symlink(dir.path().join("missing"), skills.join("old")).unwrap();
    write(skills.join("README"), "not a skill");

    let found = diagnose(&[root(&skills, Scope::Global, &[Agent::Claude])]);

    assert_eq!(
        found,
        [Diagnostic::Dangling {
            path: skills.join("old"),
            target: Some(dir.path().join("missing")),
            scope: Scope::Global,
        }]
    );
}

/// The shape of your workspace: three files claiming one name below a folder that is no skill.
fn workspace(skills: &Path) {
    let base = skills.join("git-janitor-workspace");
    write(
        base.join("iteration-3/baseline-snapshot/skill.md"),
        &skill_text("git-janitor"),
    );
    write(
        base.join("skill-snapshot/SKILL.original.md"),
        &skill_text("git-janitor"),
    );
    write(
        base.join("skill-snapshot/git-janitor.md"),
        &skill_text("git-janitor"),
    );
}

#[test]
fn skill_files_below_a_folder_that_is_no_skill_are_strays_and_collide_for_deep_scanners() {
    let dir = tempfile::tempdir().unwrap();
    let skills = dir.path().join("skills");
    workspace(&skills);

    let found = diagnose(&[root(&skills, Scope::Global, &[Agent::Codex])]);

    let strays = found
        .iter()
        .filter(|d| {
            matches!(
                d,
                Diagnostic::Stray {
                    why: Stray::WrongName,
                    ..
                }
            )
        })
        .count();
    assert_eq!(strays, 3);
    assert_eq!(
        collisions(&found),
        [("git-janitor", &Readers::Recursive, 3)]
    );
}

#[test]
fn a_skill_md_below_a_non_skill_folder_is_nested_and_a_real_skill_beside_it_is_not() {
    let dir = tempfile::tempdir().unwrap();
    let skills = dir.path().join("skills");
    skill(&skills, "tdd", "tdd");
    skill(&skills, "workspace/old/tdd", "tdd");

    let found = diagnose(&[root(&skills, Scope::Global, &[Agent::Claude])]);

    let nested = skills.join("workspace/old/tdd/SKILL.md");
    assert!(found.contains(&Diagnostic::Stray {
        path: nested,
        name: Some("tdd".into()),
        scope: Scope::Global,
        why: Stray::Nested,
    }));
    // Claude never loads the nested one, so for Claude there is nothing to collide with.
    assert_eq!(collisions(&found), [("tdd", &Readers::Recursive, 2)]);
}

#[test]
fn a_skill_like_file_directly_in_the_folder_is_loose() {
    let dir = tempfile::tempdir().unwrap();
    let skills = dir.path().join("skills");
    write(skills.join("notes.md"), "just notes, no frontmatter");
    write(skills.join("helper.md"), &skill_text("helper"));

    let found = diagnose(&[root(&skills, Scope::Global, &[Agent::Claude])]);

    assert_eq!(
        found,
        [Diagnostic::Stray {
            path: skills.join("helper.md"),
            name: Some("helper".into()),
            scope: Scope::Global,
            why: Stray::Loose,
        }]
    );
}

#[test]
fn hidden_folders_and_node_modules_are_not_searched() {
    let dir = tempfile::tempdir().unwrap();
    let skills = dir.path().join("skills");
    skill(&skills, ".system/tdd", "tdd");
    skill(&skills, "work/node_modules/pkg", "pkg");

    assert_eq!(
        diagnose(&[root(&skills, Scope::Global, &[Agent::Claude])]),
        []
    );
}

#[test]
fn a_skill_that_breaks_the_format_is_reported_with_each_rule_it_breaks() {
    let dir = tempfile::tempdir().unwrap();
    let skills = dir.path().join("skills");
    write(
        skills.join("Bad_Name/SKILL.md"),
        "---\nname: Bad_Name\n---\n",
    );
    write(skills.join("plain/SKILL.md"), "no frontmatter at all");
    skill(&skills, "fine", "fine");

    let found = diagnose(&[root(&skills, Scope::Project, &[Agent::Claude])]);

    let invalid: Vec<(&str, &[Violation])> = found
        .iter()
        .filter_map(|d| match d {
            Diagnostic::Invalid {
                name, violations, ..
            } => Some((name.as_str(), violations.as_slice())),
            _ => None,
        })
        .collect();
    assert_eq!(
        invalid,
        [
            (
                "Bad_Name",
                &[Violation::NameCharacters, Violation::MissingDescription][..]
            ),
            (
                "plain",
                &[Violation::MissingName, Violation::MissingDescription][..]
            ),
        ]
    );
}

#[cfg(unix)]
#[test]
fn a_file_reachable_through_two_roots_is_reported_once() {
    let dir = tempfile::tempdir().unwrap();
    let (store, linked) = (dir.path().join("store"), dir.path().join("linked"));
    write(store.join("Bad/SKILL.md"), "---\nname: Bad\n---\n");
    fs::create_dir_all(&linked).unwrap();
    std::os::unix::fs::symlink(store.join("Bad"), linked.join("Bad")).unwrap();

    let found = diagnose(&[
        root(&store, Scope::Global, &[Agent::Codex]),
        root(&linked, Scope::Global, &[Agent::Claude]),
    ]);

    assert_eq!(found.len(), 1);
}

#[test]
fn the_roots_are_the_folders_each_tool_reads_with_shared_ones_merged() {
    let env = Env::new("/h".into(), "/p".into());

    let roots = roots(&env);

    let shared = roots
        .iter()
        .find(|r| r.dir == Path::new("/h/.agents/skills"))
        .unwrap();
    assert_eq!(shared.agents, [Agent::Codex]);
    let claude = roots
        .iter()
        .find(|r| r.dir == Path::new("/h/.claude/skills"))
        .unwrap();
    assert_eq!(
        (claude.scope, claude.agents.as_slice()),
        (Scope::Global, &[Agent::Claude][..])
    );
    assert!(
        roots
            .iter()
            .any(|r| r.dir == Path::new("/p/.claude/skills") && r.scope == Scope::Project)
    );
}

#[test]
fn a_project_that_is_the_home_directory_does_not_repeat_the_user_level_roots() {
    let dir = tempfile::tempdir().unwrap();
    let env = Env::new(dir.path().into(), dir.path().into());

    let found = roots(&env);

    let mut dirs: Vec<_> = found.iter().map(|root| root.dir.clone()).collect();
    let all = dirs.len();
    dirs.sort();
    dirs.dedup();
    assert_eq!(dirs.len(), all);
    assert!(found.iter().all(|root| root.scope == Scope::Global));
}
