use std::path::Path;

use crate::fswalk;
use crate::matcher::pattern_matches;
use crate::model::Agent;
use crate::npx_skills;
use crate::prompt::Prompter;
use crate::runner::run_checked;

use super::global::confirm_duplicates;
use super::{Ctx, InstallRequest, SkillSelection, Tree};

/// Pick skills found under the tree and install them with `npx skills add <source>`.
pub(super) fn install(
    ctx: &Ctx,
    req: &InstallRequest,
    tree: &Tree,
    agents: &[Agent],
) -> anyhow::Result<()> {
    let names = scan_available(tree.src)?;
    let Some(chosen) = choose(ctx.prompter, &req.selection, &names, tree.source)? else {
        return Ok(());
    };
    if !confirm_duplicates(ctx, req, (tree.source, &chosen))? {
        return Ok(());
    }
    let (program, args) = npx_skills::add_command(tree.source, &chosen, agents, req.scope);
    run_checked(ctx.runner, &program, &args, &ctx.env.project)
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
fn select_skills(names: &[String], selection: &SkillSelection) -> Vec<String> {
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

/// Frontmatter names of every skill (dir containing `SKILL.md`) under `root`, recursively,
/// sorted: the names npx `skills --skill` matches.
fn scan_available(root: &Path) -> anyhow::Result<Vec<String>> {
    let mut found = Vec::new();
    fswalk::walk(root, &mut |path, is_dir| {
        if is_dir {
            found.extend(skill_name(path));
        }
    })?;
    found.sort();
    found.dedup();
    Ok(found)
}

/// The name of the skill in `dir`, unless npx would parse it as a flag.
fn skill_name(dir: &Path) -> Option<String> {
    let md = std::fs::read_to_string(dir.join("SKILL.md")).ok()?;
    let name = frontmatter_name(&md)?;
    (!name.starts_with('-')).then(|| name.to_string())
}

/// `name:` from the leading `---` frontmatter block; `None` (npx skips the skill) unless both
/// `name` and `description` are set.
/// A `description:` with no inline value (`>`, `|`, or nothing) counts once an indented line
/// follows it, as in YAML.
/// shortcut: top-level keys only, no full YAML; add a parser if real skills still go missing.
fn frontmatter_name(md: &str) -> Option<&str> {
    let mut lines = md.lines();
    if lines.next()? != "---" {
        return None;
    }
    let (mut name, mut description, mut continued) = ("", "", false);
    for line in lines {
        if line == "---" {
            return (!name.is_empty() && !description.is_empty()).then_some(name);
        }
        if continued && line.starts_with([' ', '\t']) && !line.trim().is_empty() {
            description = line.trim();
        }
        continued = false;
        match line.split_once(':') {
            Some(("name", v)) => name = unquote(v),
            Some(("description", v)) => {
                description = unquote(v);
                if description.is_empty() || BLOCK.contains(&description) {
                    (description, continued) = ("", true);
                }
            }
            _ => {}
        }
    }
    None
}

const BLOCK: [&str; 6] = [">", "|", ">-", "|-", ">+", "|+"];

fn unquote(v: &str) -> &str {
    let v = v.trim();
    ['"', '\'']
        .iter()
        .find_map(|q| v.strip_prefix(*q)?.strip_suffix(*q))
        .unwrap_or(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::test_support::*;

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
        install(&ctx, req, &tree, &req.agents)
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
        run(&fx, &request(&fx, sel, Agent::Cursor), &runner, &prompter).unwrap();
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
                "cursor",
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
    fn scan_available_recurses_skips_and_dedupes() {
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
        assert_eq!(scan_available(root).unwrap(), s(&["alpha", "beta"]));
    }

    #[test]
    fn scan_available_names_skills_by_frontmatter() {
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
            scan_available(root).unwrap(),
            s(&["folded", "plain", "q-name", "real-name", "single-q"])
        );
    }

    #[test]
    fn scan_available_skips_skills_missing_name_or_description() {
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
        assert_eq!(scan_available(root).unwrap(), s(&["ok"]));
    }

    #[test]
    fn scan_available_skips_option_like_names() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        skill_md(root, "g", "---\nname: -g\ndescription: d\n---\n");
        skill(root, "-y");
        skill(root, "ok");
        assert_eq!(scan_available(root).unwrap(), s(&["ok"]));
    }

    #[test]
    fn scan_available_root_itself_can_be_a_skill() {
        let dir = tempfile::tempdir().unwrap();
        skill(dir.path(), "solo");
        assert_eq!(
            scan_available(&dir.path().join("solo")).unwrap(),
            s(&["solo"])
        );
    }
}
