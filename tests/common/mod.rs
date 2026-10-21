//! Fixtures shared by the integration tests: a fake home + project, scripted prompts, a
//! recording command runner and builders for the commands under test.
#![allow(dead_code)]

mod sandbox;
#[allow(unused_imports)]
pub use sandbox::*;

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use skillz::agents;
use skillz::cli::{Command, Deps, Filter, ListArgs, ResetArgs, UninstallArgs, UseArgs, run};
use skillz::env::Env;
use skillz::profile::{Profile, Store};
use skillz::prompt::Prompter;
use skillz::runner::{CommandRunner, Output};

pub type Call = (String, Vec<String>);

pub struct Scripted {
    pub pick: Option<Vec<usize>>,
    pub confirm: bool,
}

impl Scripted {
    pub fn yes() -> Self {
        Scripted {
            pick: None,
            confirm: true,
        }
    }

    pub fn declining() -> Self {
        Scripted {
            pick: None,
            confirm: false,
        }
    }

    pub fn picking(indices: Vec<usize>) -> Self {
        Scripted {
            pick: Some(indices),
            confirm: true,
        }
    }
}

impl Prompter for Scripted {
    fn multi_select(&self, _title: &str, options: &[String]) -> anyhow::Result<Vec<usize>> {
        Ok(self
            .pick
            .clone()
            .unwrap_or_else(|| (0..options.len()).collect()))
    }
    fn confirm(&self, _q: &str) -> anyhow::Result<bool> {
        Ok(self.confirm)
    }
}

#[derive(Default)]
pub struct Recorder(pub RefCell<Vec<Call>>);

impl Recorder {
    pub fn calls(&self) -> Vec<Call> {
        self.0.borrow().clone()
    }

    pub fn count(&self) -> usize {
        self.0.borrow().len()
    }
}

impl CommandRunner for Recorder {
    fn run(&self, program: &str, args: &[String], _cwd: &Path) -> anyhow::Result<Output> {
        self.0.borrow_mut().push((program.into(), args.to_vec()));
        Ok(Output {
            success: true,
            ..Default::default()
        })
    }
}

pub fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

pub fn read_json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

/// Whether anything, even a dangling link, sits at `path`.
pub fn present(path: impl AsRef<Path>) -> bool {
    fs::symlink_metadata(path).is_ok()
}

pub struct World {
    pub _tmp: tempfile::TempDir,
    pub home: PathBuf,
    pub project: PathBuf,
}

impl World {
    pub fn env(&self) -> Env {
        Env {
            home: self.home.canonicalize().unwrap(),
            project: self.project.canonicalize().unwrap(),
            claude_config: None,
            codex_home: None,
        }
    }

    pub fn claude_settings(&self) -> PathBuf {
        self.home.join(".claude/settings.json")
    }

    pub fn cursor_hooks(&self) -> PathBuf {
        self.home.join(".cursor/hooks.json")
    }

    pub fn codex_project_hooks(&self) -> PathBuf {
        self.project.join(".codex/hooks.json")
    }

    pub fn tdd_store(&self) -> PathBuf {
        self.home.join(".agents/skills/tdd")
    }

    pub fn skillz_dir(&self) -> PathBuf {
        self.home.join(".config/skillz")
    }

    pub fn profiles(&self) -> Store {
        Store::new(self.skillz_dir().join("store.json"))
    }

    pub fn profile(&self, name: &str) -> Profile {
        self.profiles().load(name).unwrap()
    }

    pub fn profile_names(&self) -> Vec<String> {
        self.profiles().names().unwrap()
    }

    pub fn write_profile(&self, name: &str, text: &str) {
        self.profiles()
            .save(&Profile::parse(name, text).unwrap())
            .unwrap();
    }

    pub fn write_skill_profile(&self, name: &str, skill: &str) {
        self.write_profile(name, &skill_profile_text(skill));
    }
}

pub fn skill_profile_text(skill: &str) -> String {
    format!("[[entry]]\nkind = \"skill\"\nsource = \"o/r\"\nnames = [\"{skill}\"]\n")
}

/// Global: an npx skill linked into Claude + Cursor, two hook tools in Claude settings,
/// a Cursor hook. Project: a Codex hook.
pub fn world() -> World {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let project = tmp.path().join("proj");
    fs::create_dir_all(&project).unwrap();

    write(
        &home.join(".agents/skills/tdd/SKILL.md"),
        "---\nname: tdd\n---\n",
    );
    write(
        &home.join(".agents/.skill-lock.json"),
        &json!({"version": 3, "skills": {"tdd": {"source": "mattpocock/skills"}}}).to_string(),
    );
    for agent_dir in [".claude/skills", ".cursor/skills"] {
        fs::create_dir_all(home.join(agent_dir)).unwrap();
        std::os::unix::fs::symlink(
            home.join(".agents/skills/tdd"),
            home.join(agent_dir).join("tdd"),
        )
        .unwrap();
    }
    write(
        &home.join(".claude/settings.json"),
        &json!({
            "model": "opus",
            "hooks": {
                "PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "rtk hook claude"}]}],
                "SessionStart": [{"matcher": "", "hooks": [
                    {"type": "command", "command": "bd prime"},
                    {"type": "command", "command": "rtk hook session"}
                ]}]
            }
        })
        .to_string(),
    );
    write(
        &home.join(".cursor/hooks.json"),
        &json!({"version": 1, "hooks": {"sessionStart": [{"command": "bd cursor-hook sessionStart"}]}}).to_string(),
    );
    write(
        &project.join(".codex/hooks.json"),
        &json!({"hooks": {"SessionStart": [{"hooks": [{"type": "command", "command": "bd codex-hook SessionStart"}]}]}}).to_string(),
    );
    World {
        _tmp: tmp,
        home,
        project,
    }
}

/// A project that has a skill (from a lock file) and a Claude hook installed in it.
pub fn world_with_project_setup() -> World {
    let w = world();
    write(
        &w.project.join(".agents/skills/tdd/SKILL.md"),
        "---\nname: tdd\n---\n",
    );
    write(
        &w.project.join("skills-lock.json"),
        &json!({"version": 3, "skills": {"tdd": {"source": "mattpocock/skills"}}}).to_string(),
    );
    fs::create_dir_all(w.project.join(".claude/skills")).unwrap();
    std::os::unix::fs::symlink(
        w.project.join(".agents/skills/tdd"),
        w.project.join(".claude/skills/tdd"),
    )
    .unwrap();
    write(
        &w.project.join(".claude/settings.local.json"),
        &json!({"hooks": {"Stop": [{"hooks": [{"type": "command", "command": "say done"}]}]}})
            .to_string(),
    );
    w
}

/// Servers in every place the agents keep them: Claude user and local scope in `.claude.json`,
/// the project's shared `.mcp.json`, and Codex's config.
pub fn mcp_world() -> World {
    let w = world();
    let proj = w.project.canonicalize().unwrap().display().to_string();
    write(
        &w.home.join(".claude.json"),
        &json!({
            "mcpServers": {"user-fs": {"type": "stdio", "command": "npx", "args": ["-y", "fs"]}},
            "projects": {proj: {"mcpServers": {
                "local-web": {"type": "http", "url": "https://x/mcp"},
                "keyed": {"type": "stdio", "command": "srv", "env": {"TOKEN": "hunter2"}}
            }}}
        })
        .to_string(),
    );
    write(
        &w.project.join(".mcp.json"),
        &json!({"mcpServers": {"shared": {"command": "node", "args": ["s.js"]}}}).to_string(),
    );
    write(
        &w.home.join(".codex/config.toml"),
        "[mcp_servers.cx]\ncommand = \"cx\"\nargs = [\"--serve\"]\n",
    );
    w
}

pub fn deps<'a>(w: &World, runner: &'a Recorder, prompter: &'a dyn Prompter) -> Deps<'a> {
    Deps {
        home: w.home.clone(),
        claude_config: None,
        codex_home: None,
        cwd: w.project.clone(),
        adapters: agents::registry().into(),
        runner,
        prompter,
        interactive: false,
        checker: std::sync::Arc::new(skillz::update::check::NoCheck),
    }
}

pub fn run_quiet(cmd: Command, d: &Deps) -> anyhow::Result<()> {
    run(cmd, d, &mut Vec::new()).map(|_| ())
}

pub fn run_captured(cmd: Command, d: &Deps) -> Vec<u8> {
    let mut out = Vec::new();
    run(cmd, d, &mut out).unwrap();
    out
}

pub fn global() -> Filter {
    Filter {
        global: true,
        ..Filter::default()
    }
}

pub fn only_mcp() -> Filter {
    Filter {
        mcp: true,
        ..Filter::default()
    }
}

pub fn uninstall(query: &str, filter: Filter, yes: bool) -> Command {
    Command::Uninstall(UninstallArgs {
        query: Some(query.into()),
        project: None,
        filter,
        yes,
        dry_run: false,
    })
}

pub fn reset(filter: Filter, keep: &[&str], dry_run: bool) -> Command {
    Command::Reset(ResetArgs {
        project: None,
        filter,
        keep: keep.iter().map(|k| k.to_string()).collect(),
        dry_run,
        force: false,
    })
}

pub fn reset_forced(filter: Filter) -> Command {
    Command::Reset(ResetArgs {
        project: None,
        filter,
        keep: vec![],
        dry_run: false,
        force: true,
    })
}

pub fn use_cmd(profile: &str, dry_run: bool) -> Command {
    Command::Use(UseArgs {
        profile: profile.into(),
        exact: false,
        project: Some(PathBuf::from(".")),
        yes: true,
        dry_run,
    })
}

pub fn use_exact_cmd(profile: &str) -> Command {
    Command::Use(UseArgs {
        exact: true,
        ..match use_cmd(profile, false) {
            Command::Use(args) => args,
            _ => unreachable!(),
        }
    })
}

pub fn list_cmd(target: Option<&str>, all: bool, json: bool) -> Command {
    Command::List(ListArgs {
        target: target.map(Into::into),
        project: None,
        all,
        json,
        filter: Filter::default(),
        interactive: false,
    })
}

pub fn list_json(w: &World, target: Option<&str>) -> Vec<Value> {
    let (r, p) = (Recorder::default(), Scripted::yes());
    let out = run_captured(list_cmd(target, false, true), &deps(w, &r, &p));
    serde_json::from_slice::<Value>(&out)
        .unwrap()
        .as_array()
        .unwrap()
        .clone()
}

pub fn names_of(items: &[Value]) -> Vec<String> {
    let mut names: Vec<String> = items
        .iter()
        .map(|i| i["name"].as_str().unwrap().to_string())
        .collect();
    names.sort();
    names
}
