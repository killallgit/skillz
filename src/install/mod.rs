//! `skillz install`: fetch the source (`source`), then let each agent install its marketplace
//! plugins (`AgentAdapter::install_plugins`) and send the rest of the skills through npx
//! `skills` (`skills`).

mod global;
mod skills;
mod source;
#[cfg(test)]
pub(crate) mod test_support;

use std::path::{Path, PathBuf};

use crate::agents::{AgentAdapter, PluginInstall, PluginOutcome, adapter_for};
use crate::env::Env;
use crate::model::{Agent, Scope};
use crate::prompt::Prompter;
use crate::runner::CommandRunner;

/// Which skills to install from a multi-skill source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillSelection {
    /// No `--skill`/`--all`: offer marketplace plugins if the source has a marketplace,
    /// else let the user pick skills.
    Interactive,
    All,
    /// Names or globs (`*commit`); matches are confirmed before installing.
    Patterns(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRequest {
    /// `org/repo`, a git URL, or a local path.
    pub source: String,
    /// What a relative local `source` is relative to: the real cwd, not the `-C` project.
    pub cwd: PathBuf,
    pub agents: Vec<Agent>,
    pub scope: Scope,
    pub selection: SkillSelection,
}

/// What an install talks to: the registered agents and the outside world.
pub struct Ctx<'a> {
    pub adapters: &'a [Box<dyn AgentAdapter>],
    pub env: &'a Env,
    pub runner: &'a dyn CommandRunner,
    pub prompter: &'a dyn Prompter,
}

/// `src` is the tree to inspect; `source` is what we hand to `npx`/`claude` (never a tmp clone).
struct Tree<'a> {
    src: &'a Path,
    source: &'a str,
}

pub fn install(ctx: &Ctx, req: &InstallRequest) -> anyhow::Result<()> {
    let fetch = source::Fetch {
        source: &req.source,
        cwd: &req.cwd,
        project: &ctx.env.project,
        runner: ctx.runner,
    };
    source::with_source(&fetch, |src, source| {
        install_from(ctx, req, &Tree { src, source })
    })
}

fn install_from(ctx: &Ctx, req: &InstallRequest, tree: &Tree) -> anyhow::Result<()> {
    let rest = if req.selection == SkillSelection::Interactive {
        match offer_plugins(ctx, req, tree)? {
            Some(rest) => rest,
            None => return Ok(()),
        }
    } else {
        req.agents.clone()
    };
    if rest.is_empty() {
        return Ok(());
    }
    skills::install(ctx, req, tree, &rest)
}

/// Let each requested agent install the source's marketplace plugins. Returns the agents that
/// have no plugin install and still need their skills installed, or `None` when the user
/// declined the plugins.
fn offer_plugins(
    ctx: &Ctx,
    req: &InstallRequest,
    tree: &Tree,
) -> anyhow::Result<Option<Vec<Agent>>> {
    let plugin_req = PluginInstall {
        src: tree.src,
        source: tree.source,
        scope: req.scope,
        env: ctx.env,
        runner: ctx.runner,
        prompter: ctx.prompter,
    };
    let mut rest = vec![];
    for &agent in &req.agents {
        let outcome = match adapter_for(ctx.adapters, agent) {
            Some(a) => a.install_plugins(&plugin_req)?,
            None => PluginOutcome::Unsupported,
        };
        match outcome {
            PluginOutcome::Unsupported => rest.push(agent),
            PluginOutcome::Declined => return Ok(None),
            PluginOutcome::Installed => {}
        }
    }
    Ok(Some(rest))
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;
    use crate::agents::registry;

    fn run_install(
        fx: &Fixture,
        req: &InstallRequest,
        runner: &FakeRunner,
        prompter: &ScriptedPrompter,
    ) -> anyhow::Result<()> {
        let adapters = registry();
        let ctx = Ctx {
            adapters: &adapters,
            env: &fx.env,
            runner,
            prompter,
        };
        install(&ctx, req)
    }

    #[test]
    fn local_path_all_installs_every_skill_from_absolute_path() {
        let fx = project_with(skills_fixture);
        let runner = FakeRunner::new(|_| {});
        let r = req(&fx, "repo", SkillSelection::All);
        run_install(&fx, &r, &runner, &ScriptedPrompter::new(&[], true)).unwrap();
        let src = fx.env.project.join("repo").to_string_lossy().into_owned();
        let expected = npx(
            &src,
            &[
                "--skill",
                "caveman-commit",
                "--skill",
                "git-commit",
                "--skill",
                "tdd",
                "--skill",
                "zeta",
                "-a",
                "claude-code",
                "-y",
            ],
        );
        assert_eq!(
            runner.calls(),
            vec![("npx".into(), expected, fx.env.project.clone())]
        );
    }

    #[test]
    fn github_shorthand_clones_then_passes_original_source() {
        let fx = project_with(|_| {});
        let runner = FakeRunner::new(skills_fixture);
        let r = InstallRequest {
            agents: vec![Agent::Codex],
            scope: Scope::Global,
            ..req(&fx, "org/repo", SkillSelection::Interactive)
        };
        run_install(&fx, &r, &runner, &ScriptedPrompter::new(&[2], true)).unwrap();
        let calls = runner.calls();
        assert_eq!(calls.len(), 2);
        let (prog, args, cwd) = &calls[0];
        assert_eq!(prog, "git");
        assert_eq!(
            args[..5],
            s(&[
                "clone",
                "--depth",
                "1",
                "--",
                "https://github.com/org/repo.git"
            ])
        );
        assert_eq!(cwd, &fx.env.project);
        assert!(!Path::new(&args[5]).exists(), "tmp clone removed");
        assert_eq!(
            calls[1].1,
            npx("org/repo", &["--skill", "tdd", "-a", "codex", "-g", "-y"])
        );
    }

    #[test]
    fn marketplace_with_other_agents_offers_them_the_skill_picker() {
        let fx = project_with(marketplace_fixture);
        let runner = FakeRunner::new(|_| {});
        let r = InstallRequest {
            agents: vec![Agent::Claude, Agent::Codex],
            scope: Scope::Global,
            ..req(&fx, "repo", SkillSelection::Interactive)
        };
        let prompter = ScriptedPrompter::new(&[1], true);
        run_install(&fx, &r, &runner, &prompter).unwrap();
        assert_eq!(prompter.offered.borrow()[1], s(&["alpha", "beta"]));
        let calls = runner.calls();
        assert_eq!(
            calls[1].1,
            s(&["plugin", "install", "p2@mp", "--scope", "user"])
        );
        let src = fx.env.project.join("repo").to_string_lossy().into_owned();
        assert_eq!(
            calls[2].1,
            npx(&src, &["--skill", "beta", "-a", "codex", "-g", "-y"])
        );
        assert_eq!(calls.len(), 3);
    }

    #[test]
    fn marketplace_ignored_without_claude_or_with_explicit_selection() {
        for (agents, sel) in [
            (vec![Agent::Codex], SkillSelection::Interactive),
            (vec![Agent::Claude], SkillSelection::All),
        ] {
            let fx = project_with(marketplace_fixture);
            let runner = FakeRunner::new(|_| {});
            let r = InstallRequest {
                agents: agents.to_vec(),
                ..req(&fx, "repo", sel)
            };
            run_install(&fx, &r, &runner, &ScriptedPrompter::new(&[0], true)).unwrap();
            let calls = runner.calls();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].0, "npx");
        }
    }
}
