//! `skillz install` from a local source: which `npx skills` command it runs, and where.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::*;
use skillz::cli::{Command, Deps, InstallArgs};
use skillz::model::Agent;

const SKILL_MD: &str = "---\nname: tdd\ndescription: d\n---\n";

fn write_skill(repo: &Path, name: &str) {
    write(
        &repo.join(format!("skills/{name}/SKILL.md")),
        &format!("---\nname: {name}\ndescription: d\n---\n"),
    );
}

fn install_args(source: &str) -> InstallArgs {
    InstallArgs {
        source: source.into(),
        global: false,
        agents: vec![Agent::Codex],
        all: true,
        skill: vec![],
        project: None,
        yes: true,
    }
}

fn install(source: String) -> Command {
    Command::Install(install_args(&source))
}

/// The values following every `--skill` flag.
fn skill_flags(args: &[String]) -> Vec<&String> {
    args.iter()
        .zip(args.iter().skip(1))
        .filter(|(flag, _)| *flag == "--skill")
        .map(|(_, v)| v)
        .collect()
}

fn world_with_commit_skills() -> (World, String) {
    let w = world();
    let src = w.project.join("src-repo");
    for name in ["git-commit", "caveman-commit", "tdd"] {
        write_skill(&src, name);
    }
    let source = src.display().to_string();
    (w, source)
}

fn install_commit_skills_globally() -> Vec<Call> {
    let (w, source) = world_with_commit_skills();
    let (r, p) = (Recorder::default(), Scripted::yes());
    let cmd = Command::Install(InstallArgs {
        global: true,
        agents: vec![Agent::Claude],
        all: false,
        skill: vec!["*commit".into()],
        ..install_args(&source)
    });
    run_quiet(cmd, &deps(&w, &r, &p)).unwrap();
    r.calls()
}

fn install_from_home(project: impl FnOnce(&World) -> Option<PathBuf>) -> Vec<Call> {
    let w = world();
    let src = w.project.join("src-repo");
    write(&src.join("skills/tdd/SKILL.md"), SKILL_MD);
    let (r, p) = (Recorder::default(), Scripted::yes());
    let from_home = Deps {
        cwd: w.home.clone(),
        ..deps(&w, &r, &p)
    };
    let cmd = Command::Install(InstallArgs {
        project: project(&w),
        ..install_args(&src.display().to_string())
    });
    run_quiet(cmd, &from_home).unwrap();
    r.calls()
}

#[test]
fn a_local_source_is_installed_with_npx() {
    let calls = install_commit_skills_globally();

    assert_eq!(calls.last().unwrap().0, "npx");
}

#[test]
fn a_skill_pattern_selects_only_the_matching_skills() {
    let calls = install_commit_skills_globally();

    let mut chosen = skill_flags(&calls.last().unwrap().1);

    chosen.sort();
    assert_eq!(chosen, ["caveman-commit", "git-commit"]);
}

#[test]
fn global_install_passes_the_global_flag() {
    let calls = install_commit_skills_globally();

    assert!(calls.last().unwrap().1.contains(&"-g".to_string()));
}

#[test]
fn a_relative_source_resolves_against_the_cwd_not_the_project_dir() {
    let w = world();
    write(&w.project.join("src-repo/skills/tdd/SKILL.md"), SKILL_MD);
    fs::create_dir_all(w.project.parent().unwrap().join("other")).unwrap();
    let (r, p) = (Recorder::default(), Scripted::yes());
    let cmd = Command::Install(InstallArgs {
        project: Some("../other".into()),
        ..install_args("./src-repo")
    });

    run_quiet(cmd, &deps(&w, &r, &p)).unwrap();

    let calls = r.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        Path::new(&calls[0].1[3]).canonicalize().unwrap(),
        w.project.join("src-repo").canonicalize().unwrap()
    );
}

#[test]
fn installing_from_home_is_global() {
    let calls = install_from_home(|_| None);

    assert!(calls.last().unwrap().1.contains(&"-g".to_string()));
}

#[test]
fn installing_into_the_home_project_is_global() {
    let calls = install_from_home(|w| Some(w.home.clone()));

    assert!(calls.last().unwrap().1.contains(&"-g".to_string()));
}

#[test]
fn an_option_like_source_is_rejected() {
    let w = world();
    let (r, p) = (Recorder::default(), Scripted::yes());

    let res = run_quiet(
        install("--upload-pack=touch /tmp/x".into()),
        &deps(&w, &r, &p),
    );

    assert!(res.is_err());
}

#[test]
fn an_option_like_source_runs_no_command() {
    let w = world();
    let (r, p) = (Recorder::default(), Scripted::yes());

    let _ = run_quiet(
        install("--upload-pack=touch /tmp/x".into()),
        &deps(&w, &r, &p),
    );

    assert_eq!(r.count(), 0);
}
