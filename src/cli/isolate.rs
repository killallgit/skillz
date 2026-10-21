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
use super::use_profile::{Switching, Target, use_profile};
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
    let target = Target {
        env: &isolated,
        scope: Scope::Global,
    };
    let switching = Switching {
        profile: &args.profile,
        exact: false,
        dry_run: false,
    };
    use_profile(&inside, &target, &switching, out)?;
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
    fn the_command_sees_both_redirects() {
        let env = isolated_env(&Env::new("/h".into(), "/p".into()), "web");

        let names: Vec<&str> = redirects(&env).iter().map(|(name, _)| *name).collect();

        assert_eq!(names, ["CLAUDE_CONFIG_DIR", "CODEX_HOME"]);
    }
}
