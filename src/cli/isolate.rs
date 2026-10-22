//! `run`: a command in config dirs of its own. A profile is loaded into
//! `<skillz dir>/envs/<profile>/{claude,codex}` and the command starts with `CLAUDE_CONFIG_DIR`
//! and `CODEX_HOME` pointing there, so it sees that profile and nothing else, and nothing has
//! to be uninstalled to switch. The dirs are separate, so the agent logs in again once.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use anyhow::Context;

use super::args::RunArgs;
use super::deps::Deps;
use super::scope::resolve_project;
use super::session::{Change, Notice, change, with_session};
use crate::env::Env;
use crate::model::Scope;
use crate::profile;

pub(super) fn run_isolated(deps: &Deps, args: RunArgs, out: &mut dyn Write) -> anyhow::Result<i32> {
    profile::check_name(&args.profile)?;
    let env = resolve_project(deps, args.project.as_deref())?;
    let isolated = isolated_env(&env, &args.profile);
    create_dirs(&isolated)?;

    let runner = deps.runner.with_envs(redirects(&isolated));
    let runner = runner.as_deref().unwrap_or(deps.runner);
    let inside = deps.redirected(
        isolated.claude_config.clone(),
        isolated.codex_home.clone(),
        runner,
    );
    let ready = with_session(&inside, &isolated, out, false, |session, console| {
        let notice = change(session, Scope::Global, &Change::Apply(&args.profile))?;
        let ready = matches!(notice, Some(Notice::Applied(_) | Notice::UpToDate));
        console.conclude(notice)?;
        Ok(ready)
    })?;
    if !ready {
        return Ok(0);
    }
    out.flush()?;
    launch(runner, &args.command, &env)
}

/// The environment whose global config is the profile's own dirs; the project stays the real one.
fn isolated_env(env: &Env, profile: &str) -> Env {
    let root = env.skillz_dir().join("envs").join(profile);
    Env {
        claude_config: Some(root.join("claude")),
        codex_home: Some(root.join("codex")),
        ..env.clone()
    }
}

fn create_dirs(env: &Env) -> anyhow::Result<()> {
    for dir in [env.claude_dir(), env.codex_dir()] {
        fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    Ok(())
}

fn redirects(env: &Env) -> Vec<(&'static str, PathBuf)> {
    vec![
        ("CLAUDE_CONFIG_DIR", env.claude_dir()),
        ("CODEX_HOME", env.codex_dir()),
    ]
}

fn launch(
    runner: &dyn crate::runner::CommandRunner,
    command: &[String],
    env: &Env,
) -> anyhow::Result<i32> {
    let (program, args) = command.split_first().context("no command to run")?;
    runner.launch(program, args, &env.project)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declining_profile_preparation_never_launches_the_command() {
        use crate::agents::registry;
        use crate::install::test_support::{FakeRunner, ScriptedPrompter};
        use crate::model::Agent;
        use crate::profile::{Entry, Profile};

        let dir = tempfile::tempdir().unwrap();
        let env = Env::new(dir.path().into(), dir.path().into());
        let mut profile = Profile::new("web");
        profile.entries.push(Entry::Hook {
            event: "Stop".into(),
            matcher: None,
            command: "notify".into(),
            agents: vec![Agent::Claude],
        });
        profile::Store::in_env(&env).save(&profile).unwrap();
        let runner = FakeRunner::new(|_| {});
        let prompter = ScriptedPrompter::new(&[], false);
        let deps = Deps::for_test(dir.path(), registry(), (&runner, &prompter));
        let args = RunArgs {
            profile: "web".into(),
            project: None,
            yes: false,
            command: vec!["child".into()],
        };

        let code = run_isolated(&deps, args, &mut Vec::new()).unwrap();

        assert_eq!(code, 0);
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn each_profile_gets_config_dirs_of_its_own_under_skillz() {
        let env = Env::new("/h".into(), "/p".into());

        let web = isolated_env(&env, "web");

        assert_eq!(
            web.claude_dir(),
            PathBuf::from("/h/.config/skillz/envs/web/claude")
        );
        assert_eq!(
            web.codex_dir(),
            PathBuf::from("/h/.config/skillz/envs/web/codex")
        );
        assert_eq!(web.project, env.project);
    }

    #[test]
    fn a_moved_project_gets_the_same_config_dirs_because_they_are_keyed_by_profile_only() {
        let before = isolated_env(&Env::new("/h".into(), "/work/app".into()), "web");
        let after = isolated_env(&Env::new("/h".into(), "/app".into()), "web");

        assert_eq!(
            (before.claude_dir(), before.codex_dir()),
            (after.claude_dir(), after.codex_dir())
        );
    }

    #[test]
    fn the_command_sees_both_redirects() {
        let env = isolated_env(&Env::new("/h".into(), "/p".into()), "web");

        let names: Vec<&str> = redirects(&env).iter().map(|(name, _)| *name).collect();

        assert_eq!(names, ["CLAUDE_CONFIG_DIR", "CODEX_HOME"]);
    }
}
