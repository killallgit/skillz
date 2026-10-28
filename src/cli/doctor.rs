//! `skillz doctor`: what is wrong with the skills on disk. Read-only.

use std::io::Write;
use std::path::Path;

use super::args::DoctorArgs;
use super::deps::Deps;
use super::scope::resolve_project;
use crate::app::app_paths::tilde;
use crate::app::json::Json;
use crate::app::rule::{RULE_WIDTH, centered_rule};
use skillz_harness::agent_skills::{self, Diagnostic, Instance, Readers, Stray, Violation};
use skillz_harness::model::{Agent, Scope};

pub(super) fn doctor(deps: &Deps, args: &DoctorArgs, out: &mut dyn Write) -> anyhow::Result<()> {
    let env = resolve_project(deps, args.project.as_deref())?;
    let mut roots = agent_skills::roots(&env);
    if args.global {
        roots.retain(|root| root.scope == Scope::Global);
    }
    let found = agent_skills::diagnose(&roots);
    if args.json {
        return found.write_json(out);
    }
    report(&found, &env.home)
        .iter()
        .try_for_each(|line| writeln!(out, "{line}"))?;
    Ok(())
}

/// The diagnostics a person reads: a rule per kind of problem, `~` for the home directory.
fn report(found: &[Diagnostic], home: &Path) -> Vec<String> {
    if found.is_empty() {
        return vec!["No problems found with your skills.".into()];
    }
    let sections: [(&str, Vec<String>); 4] = [
        ("name collisions", collision_lines(found, home)),
        ("stray skill files", stray_lines(found, home)),
        ("format problems", invalid_lines(found, home)),
        ("broken links", dangling_lines(found, home)),
    ];
    let mut lines = Vec::new();
    for (title, body) in sections.into_iter().filter(|(_, body)| !body.is_empty()) {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push(centered_rule(title, RULE_WIDTH, "─"));
        lines.extend(body);
    }
    lines
}

fn collision_lines(found: &[Diagnostic], home: &Path) -> Vec<String> {
    let mut lines = Vec::new();
    for diagnostic in found {
        let Diagnostic::Collision {
            name,
            readers,
            copies,
        } = diagnostic
        else {
            continue;
        };
        lines.push(format!("{name}  ({})", who(readers)));
        lines.extend(copies.iter().map(|copy| copy_line(copy, home)));
    }
    lines
}

fn who(readers: &Readers) -> String {
    match readers {
        Readers::Agents(agents) => {
            let names: Vec<String> = agents.iter().map(Agent::to_string).collect();
            format!("read as skills by {}", names.join(", "))
        }
        Readers::Recursive => "seen by tools that also load nested and loose skill files".into(),
    }
}

/// A path as the user reads it: their home directory as `~`.
fn shown(path: &Path, home: &Path) -> String {
    tilde(&path.display().to_string(), home)
}

fn copy_line(copy: &Instance, home: &Path) -> String {
    let path = shown(&copy.path, home);
    match copy.stray {
        None => format!("  {path}"),
        Some(why) => format!("  {path}   [{}]", stray_reason(why)),
    }
}

fn stray_lines(found: &[Diagnostic], home: &Path) -> Vec<String> {
    found
        .iter()
        .filter_map(|d| match d {
            Diagnostic::Stray {
                path, name, why, ..
            } => Some(format!(
                "  {}   {}   [{}]",
                shown(path, home),
                name.as_deref().unwrap_or("(no name)"),
                stray_reason(*why)
            )),
            _ => None,
        })
        .collect()
}

fn stray_reason(why: Stray) -> &'static str {
    match why {
        Stray::Nested => "SKILL.md below a folder that is not a skill",
        Stray::WrongName => "skill frontmatter in a file not called SKILL.md",
        Stray::Loose => "skill file outside any skill folder",
    }
}

fn invalid_lines(found: &[Diagnostic], home: &Path) -> Vec<String> {
    let mut lines = Vec::new();
    for diagnostic in found {
        if let Diagnostic::Invalid {
            path,
            name,
            violations,
            ..
        } = diagnostic
        {
            lines.push(format!("{name}  {}", shown(path, home)));
            lines.extend(violations.iter().map(|v| format!("  {}", violation(*v))));
        }
    }
    lines
}

fn violation(violation: Violation) -> &'static str {
    match violation {
        Violation::MissingName => "has no name",
        Violation::NameTooLong => "name is longer than 64 characters",
        Violation::NameCharacters => "name uses characters other than a-z, 0-9 and hyphens",
        Violation::NameHyphens => "name starts or ends with a hyphen or has two in a row",
        Violation::NameIsNotTheDirectory => "name differs from its folder's name",
        Violation::MissingDescription => "has no description",
        Violation::DescriptionTooLong => "description is longer than 1024 characters",
        Violation::CompatibilityTooLong => "compatibility is longer than 500 characters",
    }
}

fn dangling_lines(found: &[Diagnostic], home: &Path) -> Vec<String> {
    found
        .iter()
        .filter_map(|d| match d {
            Diagnostic::Dangling { path, target, .. } => {
                let shown = |p: &Path| shown(p, home);
                Some(match target {
                    Some(target) => format!("  {}  →  {}", shown(path), shown(target)),
                    None => format!("  {}", shown(path)),
                })
            }
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use skillz_harness::agent_skills::Root;
    use std::fs;

    fn skill(dir: &Path, name: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: d\n---\n"),
        )
        .unwrap();
    }

    #[test]
    fn a_clean_set_of_skills_says_so_with_one_line() {
        assert_eq!(report(&[], Path::new("/h")).len(), 1);
    }

    #[test]
    fn problems_come_under_a_rule_per_kind_with_the_home_directory_as_tilde() {
        let home = tempfile::tempdir().unwrap();
        let skills = home.path().join(".agents/skills");
        skill(&skills.join("tdd"), "tdd");
        skill(&skills.join("work/old"), "tdd");
        skill(&skills.join("bad"), "Bad");
        let roots = [Root {
            dir: skills.clone(),
            scope: Scope::Global,
            agents: vec![Agent::Codex],
        }];

        let lines = report(&agent_skills::diagnose(&roots), home.path());

        let rules: Vec<&String> = lines.iter().filter(|l| l.starts_with('─')).collect();
        assert_eq!(rules.len(), 3);
        assert!(rules.iter().all(|r| r.chars().count() == RULE_WIDTH));
        assert!(
            lines
                .iter()
                .any(|l| l.contains("~/.agents/skills/work/old"))
        );
        assert!(
            !lines
                .iter()
                .any(|l| l.contains(&home.path().display().to_string()))
        );
    }

    #[test]
    fn a_symlinked_home_is_still_shown_as_tilde() {
        let base = tempfile::tempdir().unwrap();
        let real = base.path().join("real");
        let linked = base.path().join("home");
        std::os::unix::fs::symlink(&real, &linked).unwrap();
        let skills = real.join(".agents/skills");
        skill(&skills.join("tdd"), "tdd");
        skill(&skills.join("work/old"), "tdd");
        let runner = crate::app::install::test_support::FakeRunner::new(|_| {});
        let prompter = crate::app::install::test_support::ScriptedPrompter::new(&[], true);
        let deps = Deps::for_test(
            &linked,
            skillz_harness::agents::registry(),
            (&runner, &prompter),
        );
        let args = DoctorArgs {
            project: None,
            global: true,
            json: false,
        };
        let mut out = Vec::new();

        doctor(&deps, &args, &mut out).unwrap();

        let text = String::from_utf8(out).unwrap();
        let canonical = real.canonicalize().unwrap().display().to_string();
        assert!(!text.contains(&canonical));
        assert!(text.contains("~/.agents/skills/work/old"));
    }
}
