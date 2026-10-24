//! A fake agent, registered with nothing but its `AgentAdapter` impl, flows through discovery,
//! usage, prune, uninstall and install. If one of these needs an edit outside the adapter, an
//! agent's knowledge has leaked out of `src/agents/`.
//!
//! `Agent` is a closed enum, so the fake borrows the `Cursor` variant and no other adapter is
//! registered.

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use skillz::action::{Action, RemovalTarget};
use skillz::agents::{self, AgentAdapter, PluginOffer};
use skillz::cli::{Command, Deps, Filter, UninstallArgs, run};
use skillz::env::Env;
use skillz::install::{Ctx, InstallRequest, SkillSelection, install};
use skillz::model::{Agent, Augmentation, Details, Installer, Kind, Scope};
use skillz::prompt::Prompter;
use skillz::prune;
use skillz::runner::{CommandRunner, Output};
use skillz::usage;

const PLUGIN: &str = "fake-plug@mp";

struct Fake {
    installs: Rc<Cell<u32>>,
}

impl AgentAdapter for Fake {
    fn agent(&self) -> Agent {
        Agent::Codex
    }
    fn home_dir_names(&self) -> &'static [&'static str] {
        &["fake"]
    }
    fn skill_dirs(&self, env: &Env, scope: Scope) -> Vec<PathBuf> {
        vec![env.root(scope).join(".fake/skills")]
    }
    fn hook_files(&self, _env: &Env, _scope: Scope) -> Vec<PathBuf> {
        vec![]
    }
    fn discover(&self, env: &Env, scope: Scope) -> anyhow::Result<Vec<Augmentation>> {
        let root = env.home.join(".fake/plugins");
        if scope != Scope::Global || !root.join(PLUGIN).exists() {
            return Ok(vec![]);
        }
        Ok(vec![Augmentation {
            details: Details::Plugin {
                location: root.join(PLUGIN),
                skills: vec!["dup".into()],
            },
            hooks: Vec::new(),
            members: Vec::new(),
            id: format!("plugin:global:{PLUGIN}"),
            name: PLUGIN.into(),
            kind: Kind::Plugin,
            agents: vec![Agent::Codex],
            scope,
            group: "mp".into(),
            source: None,
            installer: Installer::Local,
            origin: Default::default(),
            children: vec![],
            removal: vec![Action::RemovePath(
                RemovalTarget::new(&root, [PLUGIN]).unwrap(),
            )],
        }])
    }
    fn usage(&self, _env: &Env) -> anyhow::Result<Vec<(String, u64)>> {
        Ok(vec![(format!("plugin:{PLUGIN}"), 1)])
    }
    fn tracks(&self, item: &Augmentation) -> bool {
        matches!(item.kind, Kind::Skill | Kind::Plugin)
    }
    fn update_steps(&self, item: &Augmentation, cwd: &Path) -> Vec<Action> {
        if item.kind != Kind::Plugin {
            return vec![];
        }
        vec![Action::required(
            "fake-agent",
            vec!["upgrade".into(), item.name.clone()],
            cwd,
        )]
    }
    fn plugin_offer(&self, _src: &Path, _scope: Scope) -> anyhow::Result<Option<PluginOffer>> {
        Ok(Some(PluginOffer {
            marketplace: "mp".into(),
            plugins: vec![PLUGIN.into()],
        }))
    }
    fn plugin_steps(
        &self,
        _offer: &PluginOffer,
        picked: &[String],
        _source: &str,
        _scope: Scope,
        env: &Env,
    ) -> Vec<Action> {
        self.installs.set(self.installs.get() + 1);
        vec![Action::required(
            "fake-agent",
            picked.to_vec(),
            &env.project,
        )]
    }
}

struct Yes;

impl Prompter for Yes {
    fn multi_select(&self, _title: &str, options: &[String]) -> anyhow::Result<Vec<usize>> {
        Ok((0..options.len()).collect())
    }
    fn confirm(&self, _question: &str) -> anyhow::Result<bool> {
        Ok(true)
    }
}

struct NoRun;

impl CommandRunner for NoRun {
    fn run(&self, program: &str, _args: &[String], _cwd: &Path) -> anyhow::Result<Output> {
        panic!("unexpected command: {program}");
    }
}

/// Allows the fake agent's own installer and nothing else.
struct OnlyFake;

impl CommandRunner for OnlyFake {
    fn run(&self, program: &str, _args: &[String], _cwd: &Path) -> anyhow::Result<Output> {
        assert_eq!(program, "fake-agent", "unexpected command");
        Ok(Output {
            success: true,
            stderr: String::new(),
        })
    }
}

struct World {
    _tmp: tempfile::TempDir,
    env: Env,
    installs: Rc<Cell<u32>>,
}

impl World {
    fn adapters(&self) -> Vec<Box<dyn AgentAdapter>> {
        vec![Box::new(Fake {
            installs: self.installs.clone(),
        })]
    }
}

/// Home with a loose skill `dup` in the fake's skill dir and the fake plugin that also ships
/// `dup`.
fn world() -> World {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let project = tmp.path().join("proj");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(home.join(".fake/skills/dup")).unwrap();
    fs::write(
        home.join(".fake/skills/dup/SKILL.md"),
        "---\nname: dup\n---\n",
    )
    .unwrap();
    fs::create_dir_all(home.join(".fake/plugins").join(PLUGIN)).unwrap();
    World {
        env: Env {
            home,
            project,
            claude_config: None,
            codex_home: None,
        },
        _tmp: tmp,
        installs: Rc::default(),
    }
}

#[test]
fn inventory_includes_the_fakes_skills_and_plugins() {
    let w = world();
    let inv = agents::inventory(&w.adapters(), &w.env, &[Scope::Global]);
    assert!(inv.warnings.is_empty(), "{:?}", inv.warnings);
    let mut found: Vec<(Kind, &str)> = inv
        .items
        .iter()
        .map(|i| (i.kind, i.name.as_str()))
        .collect();
    found.sort();
    assert_eq!(found, [(Kind::Plugin, PLUGIN), (Kind::Skill, "dup")]);
}

#[test]
fn prune_uses_the_fakes_usage_and_tracking() {
    let w = world();
    let adapters = w.adapters();
    let inv = agents::inventory(&adapters, &w.env, &[Scope::Global]);
    let used = usage::read(&adapters, &w.env).unwrap();
    let found = prune::check(&inv.items, &adapters, &used, 100 * 24 * 60 * 60 * 1000);
    let names = |items: Vec<&Augmentation>| -> Vec<String> {
        items.into_iter().map(|i| i.name.clone()).collect()
    };
    assert_eq!(names(found.duplicates), ["dup"]);
    assert_eq!(
        names(found.unused.iter().map(|(i, _)| *i).collect()),
        [PLUGIN]
    );
    assert!(found.never_used.is_empty());
}

#[test]
fn uninstall_runs_the_fakes_removal_plan() {
    let w = world();
    let deps = Deps {
        home: w.env.home.clone(),
        claude_config: None,
        codex_home: None,
        cwd: w.env.project.clone(),
        adapters: w.adapters().into(),
        runner: &NoRun,
        prompter: &Yes,
        interactive: false,
        checker: std::sync::Arc::new(skillz::update::check::NoCheck),
        scanner: None,
    };
    let cmd = Command::Uninstall(UninstallArgs {
        queries: vec![PLUGIN.into()],
        project: None,
        filter: Filter::default(),
        yes: true,
        dry_run: false,
    });
    run(cmd, &deps, &mut Vec::new()).unwrap();
    assert!(!w.env.home.join(".fake/plugins").join(PLUGIN).exists());
    assert!(w.env.home.join(".fake/skills/dup").exists());
}

#[test]
fn install_asks_the_fake_to_install_plugins_and_runs_no_other_command() {
    let w = world();
    let req = InstallRequest {
        source: "o/r".into(),
        cwd: w.env.project.clone(),
        agents: vec![Agent::Codex],
        scope: Scope::Global,
        selection: SkillSelection::Interactive,
    };
    // The clone of `o/r` would be a command `OnlyFake` refuses, so use a local source.
    let src = w.env.project.join("repo");
    fs::create_dir_all(&src).unwrap();
    let req = InstallRequest {
        source: src.to_string_lossy().into_owned(),
        ..req
    };
    let adapters = w.adapters();
    let ctx = Ctx {
        adapters: &adapters,
        env: &w.env,
        runner: &OnlyFake,
        prompter: &Yes,
    };
    install(&ctx, &req).unwrap();
    assert_eq!(w.installs.get(), 1);
}

#[test]
fn a_registered_adapter_owns_its_update_commands() {
    let world = world();
    let adapters = world.adapters();
    let inventory = agents::inventory(&adapters, &world.env, &[Scope::Global])
        .complete()
        .unwrap();
    let plugin = inventory
        .iter()
        .find(|item| item.kind == Kind::Plugin)
        .unwrap();

    let steps = skillz::update::steps(&adapters, plugin, &world.env.project);

    assert!(
        matches!(&steps[..], [Action::Run { program, args, cwd, .. }]
        if program == "fake-agent" && args == &["upgrade", PLUGIN] && cwd == &world.env.project)
    );
}
