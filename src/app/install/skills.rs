use crate::app::matcher::pattern_matches;
use crate::app::prompt::Prompter;
use skillz_harness::action::Action;
use skillz_harness::agent_skills::skill_names_under;
use skillz_harness::model::Agent;
use skillz_harness::npx::commands;

use super::global::confirm_duplicates;
use super::{Ctx, InstallRequest, SkillSelection, Tree};

/// The `npx skills add` step for the skills the user picks from the tree.
pub(super) struct SkillsPlan {
    pub line: String,
    pub step: Action,
}

/// Pick skills found under the tree to install with `npx skills add <source>`. `None` when the
/// user backed out.
pub(super) fn plan(
    ctx: &Ctx,
    req: &InstallRequest,
    tree: &Tree,
    agents: &[Agent],
) -> anyhow::Result<Option<SkillsPlan>> {
    let names = skill_names_under(tree.src)?;
    anyhow::ensure!(
        !names.is_empty(),
        "found no marketplace, plugin or skills in {}",
        tree.source
    );
    let Some(chosen) = choose(ctx.prompter, &req.selection, &names, tree.source)? else {
        return Ok(None);
    };
    if !confirm_duplicates(ctx, req, (tree.source, &chosen))? {
        return Ok(None);
    }
    let (program, args) = commands::add_command(tree.source, &chosen, agents, req.scope);
    let names: Vec<String> = agents.iter().map(ToString::to_string).collect();
    Ok(Some(SkillsPlan {
        line: format!("{}: skills {}", names.join(", "), chosen.join(", ")),
        step: Action::required(&program, args, &ctx.env.project),
    }))
}

/// `npx skills add <source>` for every skill the source has, or the ones named with `--skill`.
/// Nothing is left out and nothing is asked: the user checked `npx` to have it all.
pub(super) fn plan_all(
    ctx: &Ctx,
    req: &InstallRequest,
    tree: &Tree,
    agents: &[Agent],
) -> anyhow::Result<SkillsPlan> {
    let names = skill_names_under(tree.src)?;
    anyhow::ensure!(
        !names.is_empty(),
        "found no marketplace, plugin or skills in {}",
        tree.source
    );
    let (chosen, what) = match &req.selection {
        SkillSelection::Patterns(_) => {
            let chosen = select_skills(&names, &req.selection);
            anyhow::ensure!(
                !chosen.is_empty(),
                "no skills to install from {}",
                tree.source
            );
            let what = format!("skills {}", chosen.join(", "));
            (chosen, what)
        }
        _ => (vec!["*".to_string()], "all skills".to_string()),
    };
    let (program, args) = commands::add_command(tree.source, &chosen, agents, req.scope);
    let agents: Vec<String> = agents.iter().map(ToString::to_string).collect();
    Ok(SkillsPlan {
        line: format!("npx for {}: {what}", agents.join(", ")),
        step: Action::required(&program, args, &ctx.env.project),
    })
}

/// The skills to install, or `None` when the user backed out.
fn choose(
    prompter: &dyn Prompter,
    selection: &SkillSelection,
    names: &[String],
    source: &str,
) -> anyhow::Result<Option<Vec<String>>> {
    if *selection == SkillSelection::Interactive {
        let picked = prompter.multi_select("Skills to install", names)?;
        let chosen: Vec<String> = picked.iter().map(|&i| names[i].clone()).collect();
        return Ok((!chosen.is_empty()).then_some(chosen));
    }
    let chosen = select_skills(names, selection);
    anyhow::ensure!(!chosen.is_empty(), "no skills to install from {source}");
    let needs_confirm = matches!(selection, SkillSelection::Patterns(_));
    if needs_confirm && !prompter.confirm(&format!("Install {}?", chosen.join(", ")))? {
        return Ok(None);
    }
    Ok(Some(chosen))
}

/// Skill names picked by a non-interactive selection. `Interactive` yields every candidate.
pub(super) fn select_skills(names: &[String], selection: &SkillSelection) -> Vec<String> {
    match selection {
        SkillSelection::Patterns(patterns) => {
            let pats: Vec<&str> = patterns
                .iter()
                .flat_map(|p| p.split(','))
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .collect();
            names
                .iter()
                .filter(|n| pats.iter().any(|p| pattern_matches(p, n)))
                .cloned()
                .collect()
        }
        SkillSelection::All | SkillSelection::Interactive => names.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::install::test_support::*;

    /// Run the installer on the fixture's `repo/`, handing npx the source `"o/r"`.
    fn run(
        fx: &Fixture,
        req: &InstallRequest,
        runner: &FakeRunner,
        prompter: &ScriptedPrompter,
    ) -> anyhow::Result<()> {
        let src = fx.env.project.join("repo");
        let ctx = Ctx {
            adapters: &[],
            env: &fx.env,
            runner,
            prompter,
        };
        let tree = Tree {
            src: &src,
            source: "o/r",
        };
        match plan(&ctx, req, &tree, &req.agents)? {
            Some(skills) => crate::app::install::run(&ctx, &[skills.step]),
            None => Ok(()),
        }
    }

    fn request(fx: &Fixture, selection: SkillSelection, agent: Agent) -> InstallRequest {
        InstallRequest {
            agents: vec![agent],
            ..req(fx, "o/r", selection)
        }
    }

    #[test]
    fn npx_failure_is_err() {
        let fx = project_with(skills_fixture);
        let runner = FakeRunner::new(|_| {}).failing("npx");
        let prompter = ScriptedPrompter::new(&[], true);
        assert!(
            run(
                &fx,
                &request(&fx, SkillSelection::All, Agent::Claude),
                &runner,
                &prompter
            )
            .is_err()
        );
    }

    #[test]
    fn patterns_confirmed_install_matches() {
        let fx = project_with(skills_fixture);
        let runner = FakeRunner::new(|_| {});
        let sel = SkillSelection::Patterns(s(&["*commit, tdd"]));
        let prompter = ScriptedPrompter::new(&[], true);
        run(&fx, &request(&fx, sel, Agent::Codex), &runner, &prompter).unwrap();
        let expected = npx(
            "o/r",
            &[
                "--skill",
                "caveman-commit",
                "--skill",
                "git-commit",
                "--skill",
                "tdd",
                "-a",
                "codex",
                "-y",
            ],
        );
        assert_eq!(
            runner.calls(),
            vec![("npx".into(), expected, fx.env.project.clone())]
        );
    }

    #[test]
    fn patterns_declined_installs_nothing() {
        let fx = project_with(skills_fixture);
        let runner = FakeRunner::new(|_| {});
        let sel = SkillSelection::Patterns(s(&["tdd"]));
        let prompter = ScriptedPrompter::new(&[], false);
        run(&fx, &request(&fx, sel, Agent::Claude), &runner, &prompter).unwrap();
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn patterns_without_matches_is_err() {
        let fx = project_with(skills_fixture);
        let runner = FakeRunner::new(|_| {});
        let sel = SkillSelection::Patterns(s(&["nope*"]));
        let prompter = ScriptedPrompter::new(&[], true);
        assert!(run(&fx, &request(&fx, sel, Agent::Claude), &runner, &prompter).is_err());
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn interactive_offers_names_and_cancel_is_ok() {
        let fx = project_with(skills_fixture);
        let runner = FakeRunner::new(|_| {});
        let prompter = ScriptedPrompter::new(&[], true);
        let req = request(&fx, SkillSelection::Interactive, Agent::Claude);
        run(&fx, &req, &runner, &prompter).unwrap();
        assert_eq!(
            prompter.offered.borrow()[0],
            s(&["caveman-commit", "git-commit", "tdd", "zeta"])
        );
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn select_skills_splits_and_trims_patterns() {
        let names = s(&["a-commit", "b", "c"]);
        let sel = SkillSelection::Patterns(s(&[" *commit ,c", ""]));
        assert_eq!(select_skills(&names, &sel), s(&["a-commit", "c"]));
        assert_eq!(select_skills(&names, &SkillSelection::All), names);
    }

    #[test]
    fn skill_names_under_recurses_skips_and_dedupes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        skill(root, "b/deep/nested/beta");
        skill(root, "a/alpha");
        skill(root, "z/alpha");
        skill(root, ".git/hidden");
        skill(root, "node_modules/pkg/dep");
        for build_dir in ["dist", "build", "__pycache__"] {
            skill(root, &format!("{build_dir}/gen"));
        }
        assert_eq!(skill_names_under(root).unwrap(), s(&["alpha", "beta"]));
    }

    #[test]
    fn skill_names_under_names_skills_by_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        skill_md(
            root,
            "folder",
            "---\nname: real-name\ndescription: d\n---\n",
        );
        skill_md(
            root,
            "quoted",
            "---\r\ndescription: d\r\nname: \"q-name\"\r\n---\r\n",
        );
        skill_md(
            root,
            "single",
            "---\nname: 'single-q'\ndescription: 'x'\n---\nbody",
        );
        skill_md(
            root,
            "folded",
            "---\nname: folded\ndescription: >\n  long\n  text\nlicense: MIT\n---\n",
        );
        skill_md(
            root,
            "plain",
            "---\nname: plain\ndescription:\n  text\n---\n",
        );
        assert_eq!(
            skill_names_under(root).unwrap(),
            s(&["folded", "plain", "q-name", "real-name", "single-q"])
        );
    }

    #[test]
    fn skill_names_under_skips_skills_missing_name_or_description() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        skill_md(root, "no-desc", "---\nname: no-desc\n---\n");
        skill_md(root, "empty-desc", "---\nname: e\ndescription: \"\"\n---\n");
        skill_md(root, "no-name", "---\ndescription: d\n---\n");
        skill_md(root, "no-fence", "name: n\ndescription: d\n");
        skill_md(root, "body-only", "---\n---\nname: b\ndescription: d\n");
        skill_md(
            root,
            "empty-block",
            "---\nname: eb\ndescription: |\nlicense: MIT\n---\n",
        );
        skill(root, "ok");
        assert_eq!(skill_names_under(root).unwrap(), s(&["ok"]));
    }

    #[test]
    fn skill_names_under_skips_option_like_names() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        skill_md(root, "g", "---\nname: -g\ndescription: d\n---\n");
        skill(root, "-y");
        skill(root, "ok");
        assert_eq!(skill_names_under(root).unwrap(), s(&["ok"]));
    }

    #[test]
    fn skill_names_under_root_itself_can_be_a_skill() {
        let dir = tempfile::tempdir().unwrap();
        skill(dir.path(), "solo");
        assert_eq!(
            skill_names_under(&dir.path().join("solo")).unwrap(),
            s(&["solo"])
        );
    }
}
