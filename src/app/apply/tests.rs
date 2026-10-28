//! Applying profiles against a fake `npx skills` that installs into a temp project.

use std::cell::RefCell;
use std::fs;
use std::path::Path;

use super::*;
use crate::app::install::test_support::{
    FakeRunner, link, marketplace_fixture, mattpocock_fixture, npx, s, skill, skills_fixture,
};
use crate::app::profile::Profile;
use crate::app::stale::Disk;
use skillz_harness::Spec;
use skillz_harness::action::{Action, apply_edit};
use skillz_harness::agents::registry;
use skillz_harness::model::Agent;
use skillz_harness::runner::{CommandRunner, Output};

/// Installs the skills named by `--skill` into the project like `npx skills add` would: the
/// files, the link for Claude and the lock entry that records the source.
struct NpxFake {
    project: std::path::PathBuf,
    calls: RefCell<Vec<Vec<String>>>,
    fail: bool,
}

impl NpxFake {
    fn new(project: &Path) -> Self {
        NpxFake {
            project: project.into(),
            calls: RefCell::default(),
            fail: false,
        }
    }

    fn failing(project: &Path) -> Self {
        NpxFake {
            fail: true,
            ..Self::new(project)
        }
    }

    fn count(&self) -> usize {
        self.calls.borrow().len()
    }
}

impl CommandRunner for NpxFake {
    fn run(&self, _program: &str, args: &[String], _cwd: &Path) -> anyhow::Result<Output> {
        self.calls.borrow_mut().push(args.to_vec());
        if !self.fail && args.iter().any(|a| a == "add") {
            for pair in args.windows(2).filter(|w| w[0] == "--skill") {
                install_skill(&self.project, &pair[1]);
            }
        }
        Ok(Output {
            success: !self.fail,
            stderr: "boom".into(),
        })
    }
}

fn install_skill(root: &Path, name: &str) {
    write_skill(root, name);
    let links = root.join(".claude/skills");
    fs::create_dir_all(&links).unwrap();
    std::os::unix::fs::symlink(root.join(".agents/skills").join(name), links.join(name)).unwrap();
    let lock = root.join("skills-lock.json");
    let mut skills = fs::read_to_string(&lock)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .map_or_else(|| serde_json::json!({}), |v| v["skills"].clone());
    skills[name] = serde_json::json!({"source": "o/r"});
    fs::write(lock, serde_json::json!({"skills": skills}).to_string()).unwrap();
}

fn write_skill(root: &Path, name: &str) {
    let dir = root.join(".agents/skills").join(name);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: d\n---\n"),
    )
    .unwrap();
}

struct Fx {
    _dirs: (tempfile::TempDir, tempfile::TempDir),
    env: Env,
    adapters: Vec<Box<dyn Harness>>,
    /// Fetches skill sources at plan time: an empty clone, so every skill keeps `npx skills`.
    fetch: FakeRunner,
}

impl Fx {
    fn new() -> Self {
        let (home, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        Fx {
            env: Env::new(home.path().into(), project.path().into()),
            adapters: registry(),
            fetch: FakeRunner::new(|_| {}),
            _dirs: (home, project),
        }
    }

    fn applier(&self) -> Applier<'_> {
        Applier {
            adapters: &self.adapters,
            env: &self.env,
            scope: Scope::Project,
            probe: &Disk,
            runner: &self.fetch,
        }
    }

    fn npx(&self) -> NpxFake {
        NpxFake::new(&self.env.project)
    }

    fn plan(&self, target: &Profile, exact: bool) -> Switch {
        self.applier().plan(target, exact).unwrap()
    }

    fn run(&self, switch: &Switch, runner: &dyn CommandRunner) -> Report {
        self.applier().run(
            switch,
            runner,
            &mut Watch {
                step: &mut |_| {},
                item: &mut |_| {},
            },
        )
    }

    fn apply(&self, target: &Profile, exact: bool, runner: &dyn CommandRunner) -> Report {
        {
            let report = self.run(&self.plan(target, exact), runner);
            assert!(report.failures.is_empty());
            report
        }
    }

    fn has_skill(&self, name: &str) -> bool {
        self.env.project.join(".agents/skills").join(name).exists()
    }

    fn matches(&self, target: &Profile) -> bool {
        let installed = self.applier().installed().unwrap();
        compare(&target.resolved(), &installed).matches()
    }
}

/// A Claude profile with one skill entry per name.
fn profile(name: &str, skills: &[&str]) -> Profile {
    let mut p = Profile::new(name);
    p.agents = vec![Agent::Claude];
    p.entries = skills
        .iter()
        .map(|s| Spec::Skill {
            source: "o/r".into(),
            names: vec![s.to_string()],
            agents: vec![],
        })
        .collect();
    p
}

#[test]
fn applying_installs_what_is_missing_and_then_the_project_matches() {
    let fx = Fx::new();
    let web = profile("web", &["tdd"]);

    fx.apply(&web, false, &fx.npx());

    assert!(fx.has_skill("tdd"));
    assert!(fx.matches(&web));
}

#[test]
fn applying_never_removes_what_the_profile_does_not_name() {
    let fx = Fx::new();
    let runner = fx.npx();
    install_skill(&fx.env.project, "other");
    write_skill(&fx.env.project, "mine");

    fx.apply(&profile("web", &["tdd"]), false, &runner);

    assert!(fx.has_skill("other") && fx.has_skill("mine") && fx.has_skill("tdd"));
}

#[test]
fn applying_what_is_all_there_plans_and_runs_nothing() {
    let fx = Fx::new();
    let runner = fx.npx();
    let web = profile("web", &["tdd"]);
    fx.apply(&web, false, &runner);

    let again = fx.plan(&web, false);

    assert!(again.is_empty());
    assert_eq!(runner.count(), 1);
}

#[test]
fn only_the_missing_skills_are_installed() {
    let fx = Fx::new();
    let runner = fx.npx();
    install_skill(&fx.env.project, "tdd");

    let switch = fx.plan(&profile("web", &["tdd", "new"]), false);
    fx.run(&switch, &runner);

    assert_eq!(switch.present.len(), 1);
    assert_eq!(switch.installs.len(), 1);
    assert_eq!(runner.count(), 1);
}

#[test]
fn duplicate_entries_install_once() {
    let fx = Fx::new();
    let runner = fx.npx();

    fx.apply(&profile("a", &["tdd", "tdd"]), false, &runner);

    assert_eq!(runner.count(), 1);
}

#[test]
fn exact_removes_what_the_profile_does_not_want_and_keeps_what_it_does() {
    let fx = Fx::new();
    let runner = fx.npx();
    install_skill(&fx.env.project, "tdd");
    install_skill(&fx.env.project, "old");

    let switch = fx.plan(&profile("web", &["tdd"]), true);
    let done = fx.run(&switch, &runner);

    assert_eq!(switch.removals.len(), 1);
    assert!(done.removed.unwrap().still_installed.is_empty());
    assert!(fx.has_skill("tdd") && !fx.has_skill("old"));
}

#[test]
fn exact_flags_an_item_no_profile_could_install_again() {
    let fx = Fx::new();
    write_skill(&fx.env.project, "mine");
    fs::write(
        fx.env.project.join(".agents/skills/mine/SKILL.md"),
        "---\nname: mine\n---\n",
    )
    .unwrap();
    fs::create_dir_all(fx.env.project.join(".claude/skills")).unwrap();
    std::os::unix::fs::symlink(
        fx.env.project.join(".agents/skills/mine"),
        fx.env.project.join(".claude/skills/mine"),
    )
    .unwrap();

    let switch = fx.plan(&profile("web", &[]), true);

    assert_eq!(switch.removals.len(), 1);
    assert!(switch.is_unsourced(&switch.removals[0]));
}

#[test]
fn exact_has_nothing_to_do_when_nothing_is_extra() {
    let fx = Fx::new();
    let runner = fx.npx();
    let web = profile("web", &["tdd"]);
    fx.apply(&web, false, &runner);

    let exact = fx.plan(&web, true);

    assert!(exact.is_empty());
}

#[test]
fn a_failed_install_is_an_error() {
    let fx = Fx::new();
    let runner = NpxFake::failing(&fx.env.project);
    let switch = fx.plan(&profile("a", &["tdd"]), false);

    let result = fx.run(&switch, &runner);

    assert_eq!(result.failures.len(), 1);
}

#[test]
fn a_hook_lands_in_the_settings_and_an_exact_empty_profile_takes_it_out_again() {
    let fx = Fx::new();
    let settings = fx.env.project.join(".claude/settings.local.json");
    let mut p = Profile::new("notify");
    p.entries.push(Spec::Hook {
        event: "Stop".into(),
        matcher: None,
        command: "say done".into(),
        agents: vec![Agent::Claude],
    });
    let runner = fx.npx();

    fx.apply(&p, false, &runner);
    let loaded = fs::read_to_string(&settings).unwrap();
    fx.apply(&Profile::new("none"), true, &runner);
    let unloaded = fs::read_to_string(&settings).unwrap();

    assert!(loaded.contains("say done"));
    assert!(!unloaded.contains("say done"));
}

#[test]
fn an_unreadable_inventory_stops_planning_instead_of_guessing() {
    let fx = Fx::new();
    let settings = fx.env.project.join(".claude/settings.local.json");
    fs::create_dir_all(settings.parent().unwrap()).unwrap();
    fs::write(&settings, "{nope").unwrap();

    let planned = fx.applier().plan(&profile("web", &["tdd"]), false);

    assert!(planned.is_err());
}

#[test]
fn a_relative_local_subagent_source_matches_its_canonical_install() {
    let fx = Fx::new();
    let source = fx.env.project.join("definitions");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("reviewer.md"), "---\nname: reviewer\n---\nbody").unwrap();
    let mut profile = Profile::new("review");
    profile.entries.push(Spec::Subagent {
        source: "./definitions".into(),
        names: vec!["reviewer".into()],
        agents: vec![Agent::Claude],
    });
    fx.apply(&profile, false, &fx.npx());

    let again = fx.plan(&profile, true);

    assert!(again.is_empty());
    assert_eq!(again.present.len(), 1);
}

fn skill_entry(source: &str, names: &[&str], agents: &[Agent]) -> Spec {
    Spec::Skill {
        source: source.into(),
        names: s(names),
        agents: agents.to_vec(),
    }
}

/// The steps that install `entries` at `scope`, their sources fetched through `fetch`.
fn planned_steps(fx: &Fx, fetch: &FakeRunner, scope: Scope, entries: Vec<Spec>) -> Vec<Action> {
    let mut library = Profile::new("library");
    library.entries = entries;
    let applier = Applier {
        scope,
        runner: fetch,
        ..fx.applier()
    };
    let switch = applier.plan(&library, false).unwrap();
    switch
        .installs
        .into_iter()
        .flat_map(|(_, steps)| steps)
        .collect()
}

/// Each command among `steps`, as its program and arguments.
fn commands(steps: &[Action]) -> Vec<(String, Vec<String>)> {
    steps
        .iter()
        .filter_map(|step| match step {
            Action::Run { program, args, .. } => Some((program.clone(), args.clone())),
            _ => None,
        })
        .collect()
}

/// The `npx skills add` that installs `names` from `source` for Claude in the project.
fn npx_for_claude(source: &str, names: &[&str]) -> (String, Vec<String>) {
    let mut rest: Vec<&str> = names.iter().flat_map(|name| ["--skill", name]).collect();
    rest.extend(["-a", "claude-code", "-y"]);
    ("npx".into(), npx(source, &rest))
}

fn git_calls(fetch: &FakeRunner) -> Vec<Vec<String>> {
    fetch
        .calls()
        .into_iter()
        .filter(|(program, ..)| program == "git")
        .map(|(_, args, _)| args)
        .collect()
}

/// The project's `.codex/config.toml` as the steps' edits leave a missing one.
fn codex_project_config(fx: &Fx, steps: &[Action]) -> toml_edit::DocumentMut {
    let config = fx.env.project.join(".codex/config.toml");
    steps
        .iter()
        .fold(String::new(), |text, step| match step {
            Action::Edit { file, edit } if *file == config => {
                apply_edit(file, edit, &text).unwrap()
            }
            _ => text,
        })
        .parse()
        .unwrap()
}

/// Marketplace `mp` whose `p1` ships `alpha` and `p2` ships `beta`, beside a `gamma` no plugin
/// ships.
fn supplied_and_loose_fixture(root: &Path) {
    marketplace_fixture(root);
    fs::write(
        root.join(".claude-plugin/marketplace.json"),
        r#"{"name":"mp","plugins":[{"name":"p1","source":"./plugins/p1"},{"name":"p2","source":"./plugins/p2"}]}"#,
    )
    .unwrap();
    skill(root, "loose/gamma");
}

/// A link at `root/rel` to a file outside the source.
fn escaping_link(root: &Path, rel: &str) {
    link(root, rel, "/etc/hosts");
}

#[test]
fn a_skill_its_source_marketplace_supplies_installs_the_owning_plugin_in_the_project() {
    let fx = Fx::new();
    let fetch = FakeRunner::new(mattpocock_fixture);
    let entry = skill_entry("o/r", &["tdd"], &[Agent::Claude, Agent::Codex]);

    let steps = planned_steps(&fx, &fetch, Scope::Project, vec![entry]);

    let install = s(&[
        "plugin",
        "install",
        "mattpocock-skills@mattpocock",
        "--scope",
        "local",
    ]);
    assert!(commands(&steps).contains(&("claude".into(), install)));
    let config = codex_project_config(&fx, &steps);
    assert_eq!(
        config["plugins"]["mattpocock-skills@mattpocock"]["enabled"].as_bool(),
        Some(true)
    );
    assert!(commands(&steps).iter().all(|(program, _)| program != "npx"));
}

#[test]
fn a_skill_its_source_marketplace_supplies_installs_the_owning_plugin_for_the_user() {
    let fx = Fx::new();
    let fetch = FakeRunner::new(mattpocock_fixture);
    let entry = skill_entry("o/r", &["tdd"], &[Agent::Claude, Agent::Codex]);

    let steps = planned_steps(&fx, &fetch, Scope::Global, vec![entry]);

    let add = s(&["plugin", "add", "mattpocock-skills@mattpocock"]);
    assert!(commands(&steps).contains(&("codex".into(), add)));
    assert!(commands(&steps).iter().all(|(program, _)| program != "npx"));
}

/// The commands that install skill `names` of `o/r` for Claude in the project, its clone made
/// by `fixture`.
fn claude_commands(
    fixture: impl Fn(&Path) + 'static,
    names: &[&str],
) -> Vec<(String, Vec<String>)> {
    let fx = Fx::new();
    let fetch = FakeRunner::new(fixture);
    let entry = skill_entry("o/r", names, &[Agent::Claude]);
    commands(&planned_steps(&fx, &fetch, Scope::Project, vec![entry]))
}

#[test]
fn a_skill_from_a_source_without_a_marketplace_keeps_the_npx_command() {
    let commands = claude_commands(skills_fixture, &["tdd"]);

    assert_eq!(commands, [npx_for_claude("o/r", &["tdd"])]);
}

#[test]
fn a_skill_no_plugin_supplies_keeps_the_npx_command() {
    let commands = claude_commands(supplied_and_loose_fixture, &["gamma"]);

    assert_eq!(commands, [npx_for_claude("o/r", &["gamma"])]);
}

#[test]
fn skills_the_marketplace_supplies_only_some_of_all_keep_the_npx_command() {
    let commands = claude_commands(supplied_and_loose_fixture, &["alpha", "gamma"]);

    assert_eq!(commands, [npx_for_claude("o/r", &["alpha", "gamma"])]);
}

#[test]
fn an_agent_that_refuses_the_source_keeps_npx_and_the_others_still_install_natively() {
    let fx = Fx::new();
    let fetch = FakeRunner::new(mattpocock_fixture);
    let credentialed = "https://user:s3cret@host/o/r.git";
    let entry = skill_entry(credentialed, &["tdd"], &[Agent::Claude, Agent::Codex]);

    let steps = planned_steps(&fx, &fetch, Scope::Project, vec![entry]);

    let commands = commands(&steps);
    let install = s(&[
        "plugin",
        "install",
        "mattpocock-skills@mattpocock",
        "--scope",
        "local",
    ]);
    assert!(commands.contains(&("claude".into(), install)));
    let codex_only = npx(credentialed, &["--skill", "tdd", "-a", "codex", "-y"]);
    assert!(commands.contains(&("npx".into(), codex_only)));
}

#[test]
fn each_source_is_fetched_once_however_many_of_its_skills_are_missing() {
    let fx = Fx::new();
    let fetch = FakeRunner::new(skills_fixture);
    let entries = vec![
        skill_entry("o/r", &["tdd"], &[Agent::Claude]),
        skill_entry("o/r", &["git-commit"], &[Agent::Claude]),
        skill_entry("o/s", &["zeta"], &[Agent::Claude]),
    ];

    planned_steps(&fx, &fetch, Scope::Project, entries);

    let cloned: Vec<String> = git_calls(&fetch)
        .into_iter()
        .map(|args| args[4].clone())
        .collect();
    assert_eq!(
        cloned,
        ["https://github.com/o/r.git", "https://github.com/o/s.git"]
    );
}

#[test]
fn a_source_whose_fetch_fails_keeps_the_npx_command() {
    let fx = Fx::new();
    let fetch = FakeRunner::new(mattpocock_fixture).failing("git");
    let entry = skill_entry("o/r", &["tdd"], &[Agent::Claude]);

    let steps = planned_steps(&fx, &fetch, Scope::Project, vec![entry]);

    assert_eq!(commands(&steps), [npx_for_claude("o/r", &["tdd"])]);
}

#[test]
fn a_source_the_fetch_refuses_is_never_cloned_and_keeps_the_npx_command() {
    let fx = Fx::new();
    let fetch = FakeRunner::new(mattpocock_fixture);
    let entry = skill_entry("o/r/extra", &["tdd"], &[Agent::Claude]);

    let steps = planned_steps(&fx, &fetch, Scope::Project, vec![entry]);

    assert!(git_calls(&fetch).is_empty());
    assert_eq!(commands(&steps), [npx_for_claude("o/r/extra", &["tdd"])]);
}

#[test]
fn a_skill_whose_plugin_holds_a_symlink_leaving_the_source_keeps_npx() {
    let leaky_plugin = |root: &Path| {
        supplied_and_loose_fixture(root);
        escaping_link(root, "plugins/p1/skills/alpha/secret");
    };

    let commands = claude_commands(leaky_plugin, &["alpha"]);

    assert_eq!(commands, [npx_for_claude("o/r", &["alpha"])]);
}

#[test]
fn a_skill_with_a_copy_outside_its_plugin_that_leaves_the_source_keeps_npx() {
    let leaky_copy = |root: &Path| {
        supplied_and_loose_fixture(root);
        skill(root, "loose/alpha");
        escaping_link(root, "loose/alpha/secret");
    };

    let commands = claude_commands(leaky_copy, &["alpha"]);

    assert_eq!(commands, [npx_for_claude("o/r", &["alpha"])]);
}

#[test]
fn a_local_source_installs_natively_from_its_canonical_path_without_fetching() {
    let fx = Fx::new();
    let fetch = FakeRunner::new(|_| {});
    let local = tempfile::tempdir().unwrap();
    mattpocock_fixture(local.path());
    let canonical = local.path().canonicalize().unwrap();
    let source = local.path().to_string_lossy();
    let entry = skill_entry(&source, &["tdd"], &[Agent::Claude]);

    let steps = planned_steps(&fx, &fetch, Scope::Project, vec![entry]);

    let add = s(&[
        "plugin",
        "marketplace",
        "add",
        &canonical.to_string_lossy(),
        "--scope",
        "local",
    ]);
    assert!(fetch.calls().is_empty());
    assert!(commands(&steps).contains(&("claude".into(), add)));
}

#[test]
fn an_agent_the_marketplace_does_not_supply_keeps_npx_for_itself_alone() {
    let fx = Fx::new();
    let fetch = FakeRunner::new(|root| {
        skill(root, "skills/alpha");
        skill(root, "extras/beta");
        fs::create_dir_all(root.join(".claude-plugin")).unwrap();
        fs::write(
            root.join(".claude-plugin/marketplace.json"),
            r#"{"name":"mp","plugins":[{"name":"p","source":"./"}]}"#,
        )
        .unwrap();
        // Claude adds `extras` to its default `skills`; Codex reads `extras` instead of them.
        fs::write(
            root.join(".claude-plugin/plugin.json"),
            r#"{"name":"p","skills":"./extras"}"#,
        )
        .unwrap();
    });
    let entry = skill_entry("o/r", &["alpha"], &[Agent::Claude, Agent::Codex]);

    let steps = planned_steps(&fx, &fetch, Scope::Project, vec![entry]);

    let commands = commands(&steps);
    let install = s(&["plugin", "install", "p@mp", "--scope", "local"]);
    assert!(commands.contains(&("claude".into(), install)));
    let codex_only = npx("o/r", &["--skill", "alpha", "-a", "codex", "-y"]);
    let npx_runs: Vec<&Vec<String>> = commands
        .iter()
        .filter(|(program, _)| program == "npx")
        .map(|(_, args)| args)
        .collect();
    assert_eq!(npx_runs, [&codex_only]);
}
