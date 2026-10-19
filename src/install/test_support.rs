//! Fakes and fixtures shared by the install tests.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

use crate::env::Env;
use crate::model::{Agent, Scope};
use crate::prompt::Prompter;
use crate::runner::{CommandRunner, Output};

use super::{InstallRequest, SkillSelection};

pub type Call = (String, Vec<String>, PathBuf);

/// Records calls; `git clone` materialises `fixture` into the target dir.
pub struct FakeRunner {
    calls: RefCell<Vec<Call>>,
    fail: Option<&'static str>,
    fixture: Box<dyn Fn(&Path)>,
}

impl FakeRunner {
    pub fn new(fixture: impl Fn(&Path) + 'static) -> Self {
        FakeRunner {
            calls: RefCell::default(),
            fail: None,
            fixture: Box::new(fixture),
        }
    }
    pub fn failing(mut self, program: &'static str) -> Self {
        self.fail = Some(program);
        self
    }
    pub fn calls(&self) -> Vec<Call> {
        self.calls.borrow().clone()
    }
}

impl CommandRunner for FakeRunner {
    fn run(&self, program: &str, args: &[String], cwd: &Path) -> anyhow::Result<Output> {
        self.calls
            .borrow_mut()
            .push((program.into(), args.to_vec(), cwd.into()));
        let success = self.fail != Some(program);
        if success && program == "git" && args.first().map(String::as_str) == Some("clone") {
            let dest = Path::new(args.last().unwrap());
            fs::create_dir_all(dest)?;
            (self.fixture)(dest);
        }
        Ok(Output {
            success,
            stdout: String::new(),
            stderr: "boom".into(),
        })
    }
}

pub struct ScriptedPrompter {
    picks: Vec<usize>,
    confirm: bool,
    pub offered: RefCell<Vec<Vec<String>>>,
}

impl ScriptedPrompter {
    pub fn new(picks: &[usize], confirm: bool) -> Self {
        ScriptedPrompter {
            picks: picks.to_vec(),
            confirm,
            offered: RefCell::default(),
        }
    }
}

impl Prompter for ScriptedPrompter {
    fn multi_select(&self, _title: &str, options: &[String]) -> anyhow::Result<Vec<usize>> {
        self.offered.borrow_mut().push(options.to_vec());
        Ok(self.picks.clone())
    }
    fn confirm(&self, _question: &str) -> anyhow::Result<bool> {
        Ok(self.confirm)
    }
}

pub fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

pub fn skill_md(root: &Path, rel: &str, md: &str) {
    let dir = root.join(rel);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("SKILL.md"), md).unwrap();
}

/// A skill dir whose frontmatter `name:` is its folder name.
pub fn skill(root: &Path, rel: &str) {
    let name = Path::new(rel).file_name().unwrap().to_string_lossy();
    skill_md(
        root,
        rel,
        &format!("---\nname: {name}\ndescription: d\n---\n"),
    );
}

pub fn skills_fixture(root: &Path) {
    for rel in [
        "skills/tdd",
        "skills/git-commit",
        "skills/caveman-commit",
        "other/zeta",
    ] {
        skill(root, rel);
    }
}

pub fn marketplace_fixture(root: &Path) {
    skill(root, "plugins/p1/skills/alpha");
    fs::create_dir_all(root.join(".claude-plugin")).unwrap();
    fs::write(
        root.join(".claude-plugin/marketplace.json"),
        r#"{"name":"mp","plugins":[{"name":"p1"},{"name":"p2"}]}"#,
    )
    .unwrap();
}

pub struct Fixture {
    _home: tempfile::TempDir,
    _project: tempfile::TempDir,
    pub env: Env,
}

/// Env whose project contains `repo/` populated by `f`.
pub fn project_with(f: impl Fn(&Path)) -> Fixture {
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    f(&project.path().join("repo"));
    let env = Env {
        home: home.path().into(),
        project: project.path().into(),
    };
    Fixture {
        _home: home,
        _project: project,
        env,
    }
}

pub fn req(
    source: &str,
    agents: &[Agent],
    scope: Scope,
    selection: SkillSelection,
) -> InstallRequest {
    InstallRequest {
        source: source.into(),
        agents: agents.to_vec(),
        scope,
        selection,
    }
}

pub fn npx(source: &str, rest: &[&str]) -> Vec<String> {
    let mut v = s(&["-y", "skills", "add", source]);
    v.extend(s(rest));
    v
}
