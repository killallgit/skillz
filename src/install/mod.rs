//! Install a local directory or fetched source through the agents’ plugin installers and npx skills.

mod global;
mod skills;
#[cfg(test)]
pub(crate) mod test_support;

use std::path::{Path, PathBuf};

use crate::agents::{AgentAdapter, adapter_for};
use crate::env::Env;
use crate::harness::action::{self, Action};
use crate::harness::runner::CommandRunner;
use crate::model::{Agent, Scope};
use crate::prompt::Prompter;
use crate::review::{Group, PlanView, Verdict};

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
    /// What a relative local `source` is relative to: the real cwd, not the `-p` project.
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

/// What an install will do, shown for approval before anything runs.
struct Plan {
    lines: Vec<String>,
    steps: Vec<Action>,
}

/// `true` when the plan was approved and ran.
pub fn install(ctx: &Ctx, req: &InstallRequest) -> anyhow::Result<bool> {
    crate::harness::clone::with_source(&req.source, &req.cwd, ctx.runner, |src, source| {
        let tree = Tree { src, source };
        let Some(plan) = plan(ctx, req, &tree)? else {
            return Ok(false);
        };
        let view = PlanView {
            heading: format!("Install from {source}"),
            groups: vec![(Group::Install, plan.lines)],
            steps: plan.steps.iter().map(Action::describe).collect(),
            targets: None,
        };
        match ctx.prompter.plan(&view)? {
            Verdict::Apply => run(ctx, &plan.steps).map(|()| true),
            Verdict::DryRun | Verdict::Cancel | Verdict::Retarget(_) => Ok(false),
        }
    })
}

/// Each requested agent installs the source's plugins through its own installer when it offers
/// any; the agents left over install the source's skills with `npx skills`. `None` when the
/// user picked nothing.
fn plan(ctx: &Ctx, req: &InstallRequest, tree: &Tree) -> anyhow::Result<Option<Plan>> {
    let mut plan = Plan {
        lines: vec![],
        steps: vec![],
    };
    let mut rest = vec![];
    for &agent in &req.agents {
        match agent_plugins(ctx, req, tree, agent)? {
            AgentPlugins::Install { line, steps } => {
                plan.lines.push(line);
                plan.steps.extend(steps);
            }
            AgentPlugins::Declined => return Ok(None),
            AgentPlugins::Unoffered => rest.push(agent),
        }
    }
    if !rest.is_empty()
        && let Some(skills) = skills::plan(ctx, req, tree, &rest)?
    {
        plan.lines.push(skills.line);
        plan.steps.push(skills.step);
    }
    Ok((!plan.steps.is_empty()).then_some(plan))
}

/// What one agent does with the plugins a source offers.
enum AgentPlugins {
    /// The source offers it none, so its skills go through `npx skills`.
    Unoffered,
    /// The user picked no plugins.
    Declined,
    Install {
        line: String,
        steps: Vec<Action>,
    },
}

fn agent_plugins(
    ctx: &Ctx,
    req: &InstallRequest,
    tree: &Tree,
    agent: Agent,
) -> anyhow::Result<AgentPlugins> {
    if req.selection != SkillSelection::Interactive {
        return Ok(AgentPlugins::Unoffered);
    }
    let Some(adapter) = adapter_for(ctx.adapters, agent) else {
        return Ok(AgentPlugins::Unoffered);
    };
    let Some(offer) = adapter.plugin_offer(tree.src, req.scope)? else {
        return Ok(AgentPlugins::Unoffered);
    };
    let title = format!("{agent} plugins to install");
    let picked = ctx.prompter.multi_select(&title, &offer.plugins)?;
    if picked.is_empty() {
        return Ok(AgentPlugins::Declined);
    }
    let chosen: Vec<String> = picked.iter().map(|&i| offer.plugins[i].clone()).collect();
    Ok(AgentPlugins::Install {
        line: format!("{agent}: {} from {}", chosen.join(", "), offer.marketplace),
        steps: adapter.plugin_steps(&offer, &chosen, tree.source, req.scope, ctx.env),
    })
}

/// Quietly, so a screen the prompter holds is not scribbled over.
fn run(ctx: &Ctx, steps: &[Action]) -> anyhow::Result<()> {
    let progress = ctx.prompter.progress("Installing", steps.len());
    action::execute_with(steps, ctx.runner, &mut |step| {
        progress.step(&step.describe());
    })?;
    Ok(())
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
    ) -> anyhow::Result<bool> {
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
        let src = fx
            .env
            .project
            .join("repo")
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned();
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
        let src = fx
            .env
            .project
            .join("repo")
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned();
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

    #[test]
    fn codex_marketplace_installs_through_codex_plugins() {
        let fx = project_with(codex_marketplace_fixture);
        let runner = FakeRunner::new(|_| {});
        let r = InstallRequest {
            agents: vec![Agent::Codex],
            scope: Scope::Global,
            ..req(&fx, "repo", SkillSelection::Interactive)
        };
        run_install(&fx, &r, &runner, &ScriptedPrompter::new(&[1], true)).unwrap();
        let calls = runner.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "codex");
        assert_eq!(calls[0].1[..3], s(&["plugin", "marketplace", "add"]));
        assert_eq!(calls[1].1, s(&["plugin", "add", "p2@mp"]));
    }

    #[test]
    fn codex_plugins_are_user_level_so_a_project_install_falls_back_to_skills() {
        let fx = project_with(codex_marketplace_fixture);
        let runner = FakeRunner::new(|_| {});
        let r = InstallRequest {
            agents: vec![Agent::Codex],
            scope: Scope::Project,
            ..req(&fx, "repo", SkillSelection::Interactive)
        };
        run_install(&fx, &r, &runner, &ScriptedPrompter::new(&[0], true)).unwrap();
        let calls = runner.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "npx");
    }

    #[test]
    fn each_agent_installs_what_the_source_offers_it() {
        let fx = project_with(|root| {
            marketplace_fixture(root);
            codex_marketplace_fixture(root);
        });
        let runner = FakeRunner::new(|_| {});
        let r = InstallRequest {
            agents: vec![Agent::Claude, Agent::Codex],
            scope: Scope::Global,
            ..req(&fx, "repo", SkillSelection::Interactive)
        };
        run_install(&fx, &r, &runner, &ScriptedPrompter::new(&[0], true)).unwrap();
        let programs: Vec<String> = runner.calls().into_iter().map(|c| c.0).collect();
        assert_eq!(programs, s(&["claude", "claude", "codex", "codex"]));
    }

    #[test]
    fn a_declined_plan_runs_nothing() {
        let fx = project_with(marketplace_fixture);
        let runner = FakeRunner::new(|_| {});
        let r = req(&fx, "repo", SkillSelection::Interactive);
        run_install(&fx, &r, &runner, &ScriptedPrompter::new(&[0], false)).unwrap();
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn a_source_with_nothing_installable_is_an_error() {
        let fx = project_with(|root| std::fs::create_dir_all(root).unwrap());
        let runner = FakeRunner::new(|_| {});
        let r = req(&fx, "repo", SkillSelection::Interactive);
        let result = run_install(&fx, &r, &runner, &ScriptedPrompter::new(&[0], true));
        assert!(result.is_err());
        assert!(runner.calls().is_empty());
    }
}
