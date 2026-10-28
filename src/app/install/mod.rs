//! Install a local directory or fetched source through the agents’ plugin installers and npx skills.

mod global;
mod skills;
#[cfg(test)]
pub(crate) mod test_support;

use std::path::{Path, PathBuf};

use crate::app::prompt::Prompter;
use crate::app::review::{Group, PlanView, Verdict};
use skillz_harness::action::{self, Action};
use skillz_harness::agents::{Harness, PluginOffer, adapter_for};
use skillz_harness::env::Env;
use skillz_harness::model::{Agent, Scope};
use skillz_harness::runner::CommandRunner;

/// Which skills to install from a multi-skill source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillSelection {
    /// No `--skill`/`--all`: offer marketplace plugins if the source has a marketplace,
    /// else let the user pick skills.
    Interactive,
    All,
    /// Names or globs (`*commit`); prefer their owning plugins, else install loose skills.
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
    pub adapters: &'a [Box<dyn Harness>],
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
    /// Agents that refused the install, with why.
    skipped: Vec<String>,
}

/// `true` when the plan was approved and ran.
pub fn install(ctx: &Ctx, req: &InstallRequest) -> anyhow::Result<bool> {
    skillz_harness::clone::with_source(&req.source, &req.cwd, ctx.runner, |src, source| {
        let tree = Tree { src, source };
        let Some(plan) = plan(ctx, req, &tree)? else {
            return Ok(false);
        };
        let view = PlanView {
            heading: format!("Install from {source}"),
            groups: [(Group::Install, plan.lines), (Group::Skip, plan.skipped)]
                .into_iter()
                .filter(|(_, lines)| !lines.is_empty())
                .collect(),
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
        skipped: vec![],
    };
    let mut rest = vec![];
    for &agent in &req.agents {
        let Some((adapter, offer)) = plugin_offer(ctx, req, tree, agent)? else {
            rest.push(agent);
            continue;
        };
        if !offer.refused.is_empty() {
            plan.skipped
                .push(escaping_symlinks(&agent.to_string(), &offer.refused));
        }
        match agent_plugins(ctx, req, tree, adapter, &offer)? {
            AgentPlugins::Install { lines, steps } => {
                plan.lines.extend(lines);
                plan.steps.extend(steps);
            }
            AgentPlugins::Refused(why) => plan.skipped.push(why),
            AgentPlugins::Declined => return Ok(None),
            AgentPlugins::Unoffered => rest.push(agent),
        }
    }
    if !rest.is_empty()
        && let Some(skills) = skills::plan(ctx, req, tree, &rest)?
    {
        plan.lines.push(skills.line);
        plan.steps.push(skills.step);
        plan.skipped.extend(skills.skipped);
    }
    // Every agent refusing leaves nothing to approve, so say why instead of doing nothing.
    anyhow::ensure!(
        !plan.steps.is_empty() || plan.skipped.is_empty(),
        "{}",
        plan.skipped.join("; ")
    );
    Ok((!plan.steps.is_empty()).then_some(plan))
}

/// The plugins the tree offers `agent`, with the adapter that installs them.
fn plugin_offer<'a>(
    ctx: &Ctx<'a>,
    req: &InstallRequest,
    tree: &Tree,
    agent: Agent,
) -> anyhow::Result<Option<(&'a dyn Harness, PluginOffer)>> {
    let Some(adapter) = adapter_for(ctx.adapters, agent) else {
        return Ok(None);
    };
    Ok(adapter
        .plugin_offer(tree.src, req.scope)?
        .map(|offer| (adapter, offer)))
}

/// The plan line for what `who` does not install because a symlink in it leaves the source.
fn escaping_symlinks(who: &str, names: &[String]) -> String {
    format!(
        "{who}: not installed, a symlink in them leaves the source: {}",
        names.join(", ")
    )
}

/// What one agent does with the plugins a source offers.
enum AgentPlugins {
    /// The source offers it none it can install, so its skills go through `npx skills`.
    Unoffered,
    /// The user picked no plugins.
    Declined,
    /// The agent will not install from this source, and says why.
    Refused(String),
    Install {
        lines: Vec<String>,
        steps: Vec<Action>,
    },
}

fn agent_plugins(
    ctx: &Ctx,
    req: &InstallRequest,
    tree: &Tree,
    adapter: &dyn Harness,
    offer: &PluginOffer,
) -> anyhow::Result<AgentPlugins> {
    if offer.plugins.is_empty() {
        return Ok(AgentPlugins::Unoffered);
    }
    let agent = adapter.agent();
    let chosen: Vec<String> = match &req.selection {
        SkillSelection::Interactive => {
            let title = format!("{agent} plugins to install");
            let picked = ctx.prompter.multi_select(&title, &offer.plugins)?;
            if picked.is_empty() {
                return Ok(AgentPlugins::Declined);
            }
            picked.iter().map(|&i| offer.plugins[i].clone()).collect()
        }
        SkillSelection::All => offer.plugins.clone(),
        SkillSelection::Patterns(_) => {
            let available = skills::source_skills(tree.src)?.offered;
            let selected = skills::select_skills(&available, &req.selection);
            let Some(plugins) = supplying_plugins(offer, &available, &selected) else {
                return Ok(AgentPlugins::Unoffered);
            };
            plugins
        }
    };
    if chosen.is_empty() {
        return Ok(AgentPlugins::Unoffered);
    }
    let named: Vec<String> = chosen.iter().map(|name| describe(offer, name)).collect();
    let mut lines = vec![format!(
        "{agent}: {} from {}",
        named.join(", "),
        offer.marketplace
    )];
    // Only a source whose every plugin says what it ships can say what is left out.
    let known = offer
        .plugins
        .iter()
        .all(|plugin| offer.skills.contains_key(plugin));
    if req.selection == SkillSelection::All && known {
        let skipped = left_out(offer, &skills::source_skills(tree.src)?.offered);
        if !skipped.is_empty() {
            lines.push(format!(
                "{agent}: not installed, no plugin supplies them (name them with --skill): {}",
                skipped.join(", ")
            ));
        }
    }
    Ok(
        match adapter.plan_add_source(offer, &chosen, tree.source, req.scope, ctx.env) {
            Ok(steps) => AgentPlugins::Install { lines, steps },
            Err(refusal) => AgentPlugins::Refused(format!("{agent}: {refusal:#}")),
        },
    )
}

/// What installing skill `names` from a fetched tree comes to, by the rule `install --skill`
/// uses: each agent whose offer supplies every name, none refused for a symlink that leaves the
/// source, installs the owning plugins; the rest keep `npx skills`.
pub(crate) struct NativeSkills {
    pub native: Vec<(Agent, Vec<Action>)>,
    pub rest: Vec<Agent>,
}

pub(crate) fn native_skills(
    adapters: &[Box<dyn Harness>],
    env: &Env,
    scope: Scope,
    (src, source): (&Path, &str),
    names: &[String],
    agents: &[Agent],
) -> anyhow::Result<NativeSkills> {
    let offered = skills::source_skills(src)?.offered;
    let plugin_steps = |adapter: &dyn Harness| -> anyhow::Result<Option<Vec<Action>>> {
        let Some(offer) = adapter.plugin_offer(src, scope)? else {
            return Ok(None);
        };
        let Some(plugins) = supplying_plugins(&offer, &offered, names) else {
            return Ok(None);
        };
        // An agent that refuses the source keeps `npx skills` for these skills.
        Ok(adapter
            .plan_add_source(&offer, &plugins, source, scope, env)
            .ok())
    };
    let mut split = NativeSkills {
        native: vec![],
        rest: vec![],
    };
    for &agent in agents {
        match adapter_for(adapters, agent)
            .map(plugin_steps)
            .transpose()?
            .flatten()
        {
            Some(steps) => split.native.push((agent, steps)),
            None => split.rest.push(agent),
        }
    }
    Ok(split)
}

/// The offer's plugins that ship skills `names`; `None` unless every name is one the source
/// `offered` and some plugin supplies it.
fn supplying_plugins(
    offer: &PluginOffer,
    offered: &[String],
    names: &[String],
) -> Option<Vec<String>> {
    let supplied = !names.is_empty()
        && names.iter().all(|name| offered.contains(name))
        && left_out(offer, names).is_empty();
    supplied.then(|| {
        offer
            .plugins
            .iter()
            .filter(|plugin| {
                offer
                    .skills
                    .get(*plugin)
                    .is_some_and(|ships| ships.iter().any(|name| names.contains(name)))
            })
            .cloned()
            .collect()
    })
}

/// The skills in `names` that none of the offer's plugins supplies: installing the plugins
/// leaves them out.
fn left_out(offer: &PluginOffer, names: &[String]) -> Vec<String> {
    names
        .iter()
        .filter(|name| {
            !offer
                .skills
                .values()
                .any(|supplied| supplied.contains(name))
        })
        .cloned()
        .collect()
}

/// A plugin's name with how many skills it brings, when the source says.
fn describe(offer: &PluginOffer, plugin: &str) -> String {
    match offer.skills.get(plugin).map(Vec::len) {
        Some(count) if count > 0 => {
            format!(
                "{plugin} ({count} skill{})",
                if count == 1 { "" } else { "s" }
            )
        }
        _ => plugin.to_string(),
    }
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
    use skillz_harness::agents::registry;

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

    fn offer(skills: &[(&str, &[&str])]) -> PluginOffer {
        PluginOffer {
            marketplace: "mp".into(),
            plugins: skills
                .iter()
                .map(|(plugin, _)| plugin.to_string())
                .collect(),
            skills: skills
                .iter()
                .map(|(plugin, names)| (plugin.to_string(), s(names)))
                .collect(),
            refused: vec![],
        }
    }

    const CREDENTIALED: &str = "https://user:s3cret@host/o/r.git";

    fn project_install(
        source: &str,
        agents: Vec<Agent>,
        selection: SkillSelection,
        prepare: impl Fn(&std::path::Path) + 'static,
    ) -> (anyhow::Result<bool>, ScriptedPrompter, Fixture) {
        let fx = project_with(|_| {});
        let runner = FakeRunner::new(prepare);
        let request = InstallRequest {
            agents,
            scope: Scope::Project,
            ..req(&fx, source, selection)
        };
        let prompter = ScriptedPrompter::new(&[0, 1], true);
        let result = run_install(&fx, &request, &runner, &prompter);
        (result, prompter, fx)
    }

    #[test]
    fn an_agent_that_refuses_the_source_is_skipped_and_the_others_still_install() {
        let (result, prompter, fx) = project_install(
            CREDENTIALED,
            vec![Agent::Claude, Agent::Codex],
            SkillSelection::All,
            marketplace_fixture,
        );

        assert!(result.unwrap());
        assert_eq!(
            prompter.plans.borrow()[0],
            [(Group::Install, 1), (Group::Skip, 1)]
        );
        assert!(!fx.env.project.join(".codex/config.toml").exists());
    }

    #[test]
    fn an_install_every_agent_refuses_is_an_error_not_a_silent_nothing() {
        let (result, prompter, _) = project_install(
            CREDENTIALED,
            vec![Agent::Codex],
            SkillSelection::All,
            marketplace_fixture,
        );

        assert!(result.is_err());
        assert!(prompter.plans.borrow().is_empty());
    }

    #[test]
    fn all_lists_the_skills_no_plugin_supplies_as_not_installed() {
        let lines = |extra: bool| {
            let (result, prompter, _) = project_install(
                "https://host/o/r.git",
                vec![Agent::Claude],
                SkillSelection::All,
                move |root| {
                    marketplace_fixture(root);
                    std::fs::write(
                        root.join(".claude-plugin/marketplace.json"),
                        r#"{"name":"mp","plugins":[{"name":"p1","source":"./plugins/p1"},{"name":"p2","source":"./plugins/p2"}]}"#,
                    )
                    .unwrap();
                    if extra {
                        skill(root, "loose/gamma");
                    }
                },
            );
            assert!(result.unwrap());
            let plans = prompter.plans.borrow();
            plans[0].clone()
        };

        assert_eq!(lines(false), [(Group::Install, 1)]);
        assert_eq!(lines(true), [(Group::Install, 2)]);
    }

    #[test]
    fn skills_no_plugin_supplies_are_the_ones_installing_plugins_leaves_out() {
        let offer = offer(&[("p1", &["alpha"]), ("p2", &["beta", "gamma"])]);

        assert_eq!(
            left_out(&offer, &s(&["alpha", "beta", "loose", "other"])),
            s(&["loose", "other"])
        );
        assert!(left_out(&offer, &s(&["alpha", "gamma"])).is_empty());
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
    fn claude_marketplaces_are_offered_as_plugins_to_both_agents() {
        let fx = project_with(marketplace_fixture);
        let runner = FakeRunner::new(|_| {});
        let r = InstallRequest {
            agents: vec![Agent::Claude, Agent::Codex],
            scope: Scope::Global,
            ..req(&fx, "repo", SkillSelection::Interactive)
        };
        let prompter = ScriptedPrompter::new(&[1], true);
        run_install(&fx, &r, &runner, &prompter).unwrap();
        assert_eq!(prompter.offered.borrow()[1], s(&["p1", "p2"]));
        let calls = runner.calls();
        assert_eq!(
            calls[1].1,
            s(&["plugin", "install", "p2@mp", "--scope", "user"])
        );
        assert_eq!(calls[3].1, s(&["plugin", "add", "p2@mp"]));
        assert_eq!(calls.len(), 4);
    }

    #[test]
    fn all_installs_every_native_plugin_without_a_picker() {
        for agent in [Agent::Claude, Agent::Codex] {
            let fx = project_with(marketplace_fixture);
            let runner = FakeRunner::new(|_| {});
            let r = InstallRequest {
                agents: vec![agent],
                scope: Scope::Global,
                ..req(&fx, "repo", SkillSelection::All)
            };
            run_install(&fx, &r, &runner, &ScriptedPrompter::new(&[0], true)).unwrap();
            let calls = runner.calls();
            assert_eq!(calls.len(), 3);
            assert!(calls.iter().all(|call| call.0 == agent.to_string()));
            assert!(calls[1].1.contains(&"p1@mp".into()));
            assert!(calls[2].1.contains(&"p2@mp".into()));
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
    fn codex_project_source_installs_configure_native_plugins() {
        let fx = project_with(codex_marketplace_fixture);
        let runner = FakeRunner::new(|_| {});
        let r = InstallRequest {
            agents: vec![Agent::Codex],
            scope: Scope::Project,
            ..req(&fx, "repo", SkillSelection::Interactive)
        };
        run_install(&fx, &r, &runner, &ScriptedPrompter::new(&[0], true)).unwrap();
        assert!(runner.calls().is_empty());
        let config: toml_edit::DocumentMut =
            std::fs::read_to_string(fx.env.project.join(".codex/config.toml"))
                .unwrap()
                .parse()
                .unwrap();
        assert_eq!(config["plugins"]["p1@mp"]["enabled"].as_bool(), Some(true));
        assert!(!fx.env.home.join(".codex/config.toml").exists());
    }

    #[test]
    fn selected_mattpocock_skills_install_the_owning_plugin_for_both_agents() {
        for scope in [Scope::Project, Scope::Global] {
            let fx = project_with(mattpocock_fixture);
            let runner = FakeRunner::new(|_| {});
            let request = InstallRequest {
                agents: vec![Agent::Claude, Agent::Codex],
                scope,
                ..req(&fx, "repo", SkillSelection::Patterns(s(&["tdd"])))
            };
            run_install(&fx, &request, &runner, &ScriptedPrompter::new(&[], true)).unwrap();
            let calls = runner.calls();
            assert!(calls.iter().all(|call| call.0 != "npx"));
            assert_eq!(
                calls[1].1,
                s(&[
                    "plugin",
                    "install",
                    "mattpocock-skills@mattpocock",
                    "--scope",
                    if scope == Scope::Project {
                        "local"
                    } else {
                        "user"
                    }
                ])
            );
            if scope == Scope::Global {
                assert_eq!(
                    calls[3].1,
                    s(&["plugin", "add", "mattpocock-skills@mattpocock"])
                );
            } else {
                let config: toml_edit::DocumentMut =
                    std::fs::read_to_string(fx.env.project.join(".codex/config.toml"))
                        .unwrap()
                        .parse()
                        .unwrap();
                assert_eq!(
                    config["plugins"]["mattpocock-skills@mattpocock"]["enabled"].as_bool(),
                    Some(true)
                );
            }
        }
    }

    #[test]
    fn selecting_a_skill_installs_only_its_owning_plugin() {
        let fx = project_with(|root| {
            marketplace_fixture(root);
            std::fs::write(root.join(".claude-plugin/marketplace.json"), r#"{"name":"mp","plugins":[{"name":"p1","source":"./plugins/p1"},{"name":"p2","source":"./plugins/p2"}]}"#).unwrap();
        });
        let runner = FakeRunner::new(|_| {});
        let request = req(&fx, "repo", SkillSelection::Patterns(s(&["alpha"])));
        run_install(&fx, &request, &runner, &ScriptedPrompter::new(&[], true)).unwrap();
        let calls = runner.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls[1].1,
            s(&["plugin", "install", "p1@mp", "--scope", "local"])
        );
    }

    #[test]
    fn skills_outside_plugins_still_install_as_loose_skills() {
        let fx = project_with(marketplace_fixture);
        let runner = FakeRunner::new(|_| {});
        let request = req(&fx, "repo", SkillSelection::Patterns(s(&["alpha"])));
        run_install(&fx, &request, &runner, &ScriptedPrompter::new(&[], true)).unwrap();
        assert_eq!(runner.calls().len(), 1);
        assert_eq!(runner.calls()[0].0, "npx");
    }

    #[test]
    fn claude_custom_skill_paths_preserve_default_skills() {
        for paths in [
            r#""./custom/custom-helper""#,
            r#"["./custom/custom-helper"]"#,
        ] {
            let fx = project_with(|root| {
                mattpocock_fixture(root);
                skill(root, "custom/custom-helper");
                std::fs::write(
                    root.join(".claude-plugin/plugin.json"),
                    format!(r#"{{"name":"mattpocock-skills","skills":{paths}}}"#),
                )
                .unwrap();
            });
            let runner = FakeRunner::new(|_| {});
            let request = req(&fx, "repo", SkillSelection::Patterns(s(&["custom-helper"])));
            run_install(&fx, &request, &runner, &ScriptedPrompter::new(&[], true)).unwrap();
            assert_eq!(runner.calls()[1].0, "claude");
            let runner = FakeRunner::new(|_| {});
            let request = req(&fx, "repo", SkillSelection::Patterns(s(&["code-review"])));
            run_install(&fx, &request, &runner, &ScriptedPrompter::new(&[], true)).unwrap();
            assert_eq!(runner.calls().len(), 2);
            assert_eq!(runner.calls()[1].0, "claude");
        }
    }

    #[test]
    fn root_entry_skill_subsets_select_only_the_matching_native_plugin() {
        let fx = project_with(|root| {
            skill(root, "skills/alpha");
            skill(root, "skills/beta");
            std::fs::create_dir_all(root.join(".claude-plugin")).unwrap();
            std::fs::write(root.join(".claude-plugin/marketplace.json"), r#"{"name":"mp","plugins":[{"name":"p1","source":"./","skills":["./skills/alpha"],"strict":false},{"name":"p2","source":"./","skills":["./skills/beta"],"strict":false}]}"#).unwrap();
        });
        let runner = FakeRunner::new(|_| {});
        let request = InstallRequest {
            agents: vec![Agent::Claude, Agent::Codex],
            ..req(&fx, "repo", SkillSelection::Patterns(s(&["alpha"])))
        };
        run_install(&fx, &request, &runner, &ScriptedPrompter::new(&[], true)).unwrap();
        let calls = runner.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls[1].1,
            s(&["plugin", "install", "p1@mp", "--scope", "local"])
        );
        let config: toml_edit::DocumentMut =
            std::fs::read_to_string(fx.env.project.join(".codex/config.toml"))
                .unwrap()
                .parse()
                .unwrap();
        assert_eq!(config["plugins"]["p1@mp"]["enabled"].as_bool(), Some(true));
        assert!(config["plugins"].get("p2@mp").is_none());
    }

    #[test]
    fn multiple_codex_plugins_share_one_marketplace_download() {
        let fx = project_with(|_| {});
        let runner = FakeRunner::new(codex_marketplace_fixture);
        let request = InstallRequest {
            agents: vec![Agent::Codex],
            ..req(&fx, "org/repo", SkillSelection::All)
        };
        run_install(&fx, &request, &runner, &ScriptedPrompter::new(&[], true)).unwrap();
        let calls = runner.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "git");
        assert_eq!(calls[1].0, "codex");
        assert!(
            calls[1]
                .1
                .windows(4)
                .any(|args| args == ["plugin", "marketplace", "upgrade", "mp"])
        );
        let config: toml_edit::DocumentMut =
            std::fs::read_to_string(fx.env.project.join(".codex/config.toml"))
                .unwrap()
                .parse()
                .unwrap();
        for plugin in ["p1@mp", "p2@mp"] {
            assert_eq!(config["plugins"][plugin]["enabled"].as_bool(), Some(true));
        }
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

    /// A link at `root/rel` to a file beside the source, outside it.
    fn escaping_link(root: &Path, rel: &str) {
        let secret = root.parent().unwrap().join("secret");
        std::fs::write(&secret, "").unwrap();
        link(root, rel, secret);
    }

    fn leaky_skills_fixture(root: &Path) {
        skills_fixture(root);
        skill(root, "skills/leaky");
        escaping_link(root, "skills/leaky/secret");
    }

    fn leaky_plugins_fixture(root: &Path) {
        skill(root, "plugins/p1/skills/alpha");
        escaping_link(root, "plugins/p1/skills/alpha/secret");
        skill(root, "plugins/p2/skills/beta");
        std::fs::create_dir_all(root.join(".claude-plugin")).unwrap();
        std::fs::write(
            root.join(".claude-plugin/marketplace.json"),
            r#"{"name":"mp","plugins":[{"name":"p1","source":"./plugins/p1"},{"name":"p2","source":"./plugins/p2"}]}"#,
        )
        .unwrap();
    }

    #[test]
    fn a_skill_with_an_escaping_symlink_is_not_offered_and_is_shown_skipped() {
        let fx = project_with(leaky_skills_fixture);
        let runner = FakeRunner::new(|_| {});
        let prompter = ScriptedPrompter::new(&[0], true);
        let request = req(&fx, "repo", SkillSelection::Interactive);

        run_install(&fx, &request, &runner, &prompter).unwrap();

        assert_eq!(
            prompter.offered.borrow()[0],
            s(&["caveman-commit", "git-commit", "tdd", "zeta"])
        );
        assert_eq!(
            prompter.plans.borrow()[0],
            [(Group::Install, 1), (Group::Skip, 1)]
        );
    }

    #[test]
    fn all_leaves_a_skill_with_an_escaping_symlink_out_of_the_npx_arguments() {
        let fx = project_with(leaky_skills_fixture);
        let runner = FakeRunner::new(|_| {});
        let request = req(&fx, "repo", SkillSelection::All);

        run_install(&fx, &request, &runner, &ScriptedPrompter::new(&[], true)).unwrap();

        let calls = runner.calls();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].1.contains(&"tdd".to_string()));
        assert!(!calls[0].1.contains(&"leaky".to_string()));
    }

    #[test]
    fn naming_only_a_skill_with_an_escaping_symlink_fails_and_runs_nothing() {
        let fx = project_with(leaky_skills_fixture);
        let runner = FakeRunner::new(|_| {});
        let prompter = ScriptedPrompter::new(&[], true);
        let request = req(&fx, "repo", SkillSelection::Patterns(s(&["leaky"])));

        let result = run_install(&fx, &request, &runner, &prompter);

        assert!(result.is_err());
        assert!(runner.calls().is_empty());
        assert!(prompter.plans.borrow().is_empty());
    }

    #[test]
    fn a_name_carried_by_one_escaping_directory_is_refused_for_all_of_them() {
        let fx = project_with(|root| {
            skill(root, "skills/tdd");
            skill(root, "clean/dup");
            skill(root, "leaky/dup");
            escaping_link(root, "leaky/dup/secret");
        });
        let runner = FakeRunner::new(|_| {});
        let prompter = ScriptedPrompter::new(&[], true);
        let request = req(&fx, "repo", SkillSelection::All);

        run_install(&fx, &request, &runner, &prompter).unwrap();

        assert!(!runner.calls()[0].1.contains(&"dup".to_string()));
        assert_eq!(
            prompter.plans.borrow()[0],
            [(Group::Install, 1), (Group::Skip, 1)]
        );
    }

    #[test]
    fn a_plugin_with_an_escaping_skill_is_refused_and_its_sibling_still_installs() {
        let fx = project_with(leaky_plugins_fixture);
        let runner = FakeRunner::new(|_| {});
        let prompter = ScriptedPrompter::new(&[0], true);
        let request = req(&fx, "repo", SkillSelection::Interactive);

        run_install(&fx, &request, &runner, &prompter).unwrap();

        assert_eq!(prompter.offered.borrow()[0], s(&["p2"]));
        assert_eq!(
            prompter.plans.borrow()[0],
            [(Group::Install, 1), (Group::Skip, 1)]
        );
        assert_eq!(
            runner.calls()[1].1,
            s(&["plugin", "install", "p2@mp", "--scope", "local"])
        );
    }

    #[test]
    fn a_source_whose_every_plugin_and_skill_is_refused_fails_and_runs_nothing() {
        let fx = project_with(|root| {
            leaky_plugins_fixture(root);
            std::fs::remove_dir_all(root.join("plugins/p2")).unwrap();
            std::fs::write(
                root.join(".claude-plugin/marketplace.json"),
                r#"{"name":"mp","plugins":[{"name":"p1","source":"./plugins/p1"}]}"#,
            )
            .unwrap();
        });
        let runner = FakeRunner::new(|_| {});
        let prompter = ScriptedPrompter::new(&[0], true);
        let request = InstallRequest {
            agents: vec![Agent::Claude, Agent::Codex],
            ..req(&fx, "repo", SkillSelection::Interactive)
        };

        let result = run_install(&fx, &request, &runner, &prompter);

        assert!(result.is_err());
        assert!(runner.calls().is_empty());
        assert!(prompter.plans.borrow().is_empty());
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
