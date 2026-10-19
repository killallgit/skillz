//! `skillz install`: fetch the source (`source`), then install its Claude marketplace
//! plugins (`marketplace`) and/or its skills via npx `skills` (`skills`).

mod marketplace;
mod skills;
mod source;
#[cfg(test)]
pub(crate) mod test_support;

use std::path::Path;

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
    pub agents: Vec<Agent>,
    pub scope: Scope,
    pub selection: SkillSelection,
}

type Cmd = (String, Vec<String>);

pub fn install(
    req: &InstallRequest,
    env: &Env,
    runner: &dyn CommandRunner,
    prompter: &dyn Prompter,
) -> anyhow::Result<()> {
    source::with_source(&req.source, env, runner, |src, source| {
        install_from(req, src, source, env, runner, prompter)
    })
}

/// `src` is the tree to inspect; `source` is what we hand to `npx`/`claude` (never a tmp clone).
fn install_from(
    req: &InstallRequest,
    src: &Path,
    source: &str,
    env: &Env,
    runner: &dyn CommandRunner,
    prompter: &dyn Prompter,
) -> anyhow::Result<()> {
    let skills = |selection: &SkillSelection, agents: &[Agent]| {
        skills::install(
            selection, agents, req.scope, src, source, env, runner, prompter,
        )
    };
    let mp = match marketplace::read(src)? {
        Some(mp)
            if req.selection == SkillSelection::Interactive
                && req.agents.contains(&Agent::Claude) =>
        {
            mp
        }
        _ => return skills(&req.selection, &req.agents),
    };
    if !marketplace::install(&mp, source, req.scope, env, runner, prompter)? {
        return Ok(());
    }
    let others: Vec<Agent> = req
        .agents
        .iter()
        .copied()
        .filter(|a| *a != Agent::Claude)
        .collect();
    if others.is_empty() {
        return Ok(());
    }
    skills(&SkillSelection::All, &others)
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn local_path_all_installs_every_skill_from_absolute_path() {
        let fx = project_with(skills_fixture);
        let runner = FakeRunner::new(|_| {});
        let r = req(
            "repo",
            &[Agent::Claude],
            Scope::Project,
            SkillSelection::All,
        );
        install(&r, &fx.env, &runner, &ScriptedPrompter::new(&[], true)).unwrap();
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
        let r = req(
            "org/repo",
            &[Agent::Codex],
            Scope::Global,
            SkillSelection::Interactive,
        );
        install(&r, &fx.env, &runner, &ScriptedPrompter::new(&[2], true)).unwrap();
        let calls = runner.calls();
        assert_eq!(calls.len(), 2);
        let (prog, args, cwd) = &calls[0];
        assert_eq!(prog, "git");
        assert_eq!(
            args[..4],
            s(&["clone", "--depth", "1", "https://github.com/org/repo.git"])
        );
        assert_eq!(cwd, &fx.env.project);
        assert!(!Path::new(&args[4]).exists(), "tmp clone removed");
        assert_eq!(
            calls[1].1,
            npx("org/repo", &["--skill", "tdd", "-a", "codex", "-g", "-y"])
        );
    }

    #[test]
    fn marketplace_with_other_agents_also_installs_all_skills_for_them() {
        let fx = project_with(marketplace_fixture);
        let runner = FakeRunner::new(|_| {});
        let r = req(
            "repo",
            &[Agent::Claude, Agent::Codex],
            Scope::Global,
            SkillSelection::Interactive,
        );
        install(&r, &fx.env, &runner, &ScriptedPrompter::new(&[1], true)).unwrap();
        let calls = runner.calls();
        assert_eq!(
            calls[1].1,
            s(&["plugin", "install", "p2@mp", "--scope", "user"])
        );
        let src = fx.env.project.join("repo").to_string_lossy().into_owned();
        assert_eq!(
            calls[2].1,
            npx(&src, &["--skill", "alpha", "-a", "codex", "-g", "-y"])
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
            let r = req("repo", &agents, Scope::Project, sel);
            install(&r, &fx.env, &runner, &ScriptedPrompter::new(&[0], true)).unwrap();
            let calls = runner.calls();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].0, "npx");
        }
    }
}
