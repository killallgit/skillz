//! Fakes and fixtures for tests of code that runs commands and reads a project: shared by this
//! crate's tests and the app's.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

use crate::env::Env;
use crate::runner::{CommandRunner, Output};

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
            stderr: "boom".into(),
        })
    }
}

pub const LOCKED_LINK: &str = "https://example.test/o/r.git";

/// Stands in for `npx skills add` run in a project: leaves the `tdd` skill and a lock entry
/// with a link and a hash behind, as npx does, and notes the arguments.
#[derive(Default)]
pub struct FakeNpx {
    pub calls: RefCell<Vec<Vec<String>>>,
}

impl FakeNpx {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CommandRunner for FakeNpx {
    fn run(&self, _program: &str, args: &[String], cwd: &Path) -> anyhow::Result<Output> {
        self.calls.borrow_mut().push(args.to_vec());
        skill(cwd, ".agents/skills/tdd");
        fs::create_dir_all(cwd.join(".claude/skills"))?;
        let link = cwd.join(".claude/skills/tdd");
        if !link.exists() {
            std::os::unix::fs::symlink(cwd.join(".agents/skills/tdd"), link)?;
        }
        let lock = serde_json::json!({"skills": {"tdd": {
            "source": "o/r", "sourceUrl": LOCKED_LINK, "computedHash": "abc123"
        }}});
        fs::write(cwd.join("skills-lock.json"), lock.to_string())?;
        Ok(Output {
            success: true,
            stderr: String::new(),
        })
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

/// A symlink at `root/rel` to `target`, with the directories above it.
pub fn link(root: &Path, rel: &str, target: impl AsRef<Path>) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(target, path).unwrap();
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
    skill(root, "plugins/p2/skills/beta");
    fs::create_dir_all(root.join(".claude-plugin")).unwrap();
    fs::write(
        root.join(".claude-plugin/marketplace.json"),
        r#"{"name":"mp","plugins":[{"name":"p1"},{"name":"p2"}]}"#,
    )
    .unwrap();
}

/// A Codex marketplace `mp` with plugins `p1` and `p2`, and the skills they ship.
pub fn codex_marketplace_fixture(root: &Path) {
    skill(root, "plugins/p1/skills/alpha");
    skill(root, "plugins/p2/skills/beta");
    fs::create_dir_all(root.join(".agents/plugins")).unwrap();
    fs::write(
        root.join(".agents/plugins/marketplace.json"),
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
        claude_config: None,
        codex_home: None,
    };
    Fixture {
        _home: home,
        _project: project,
        env,
    }
}

pub fn npx(source: &str, rest: &[&str]) -> Vec<String> {
    let mut v = s(&["-y", "skills", "add", source]);
    v.extend(s(rest));
    v
}
