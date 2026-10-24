use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::SkillDir;
use crate::action::{Action, Edit, RemovalTarget};
use crate::env::Env;
use crate::model::{Agent, Installed, Scope};

pub(super) struct Fixture {
    _tmp: TempDir,
    pub env: Env,
}

pub(super) fn fixture() -> Fixture {
    let tmp = TempDir::new().unwrap();
    let root = fs::canonicalize(tmp.path()).unwrap();
    let env = Env {
        home: root.join("home"),
        project: root.join("proj"),
        claude_config: None,
        codex_home: None,
    };
    fs::create_dir_all(&env.home).unwrap();
    fs::create_dir_all(&env.project).unwrap();
    Fixture { _tmp: tmp, env }
}

/// A removal plan without its native-tool commands.
pub(super) fn files(plan: &[Action]) -> Vec<Action> {
    plan.iter()
        .filter(|a| !matches!(a, Action::Run { .. }))
        .cloned()
        .collect()
}

pub(super) fn skill(dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("SKILL.md"), "---\nname: x\n---\n").unwrap();
}

pub(super) fn link(target: &Path, at: &Path) {
    fs::create_dir_all(at.parent().unwrap()).unwrap();
    symlink(target, at).unwrap();
}

pub(super) fn sd(agent: Agent, scope: Scope, path: PathBuf) -> SkillDir {
    SkillDir { agent, scope, path }
}

pub(super) fn global_dirs(env: &Env) -> Vec<SkillDir> {
    let h = &env.home;
    vec![
        sd(Agent::Claude, Scope::Global, h.join(".claude/skills")),
        sd(Agent::Codex, Scope::Global, h.join(".agents/skills")),
        sd(Agent::Codex, Scope::Global, h.join(".codex/skills")),
    ]
}

pub(super) fn rm(p: PathBuf) -> Action {
    let target = RemovalTarget::new(p.parent().unwrap(), [p.file_name().unwrap()]);
    Action::RemovePath(target.unwrap())
}

pub(super) fn lock_edit(file: PathBuf, name: &str) -> Action {
    Action::Edit {
        file,
        edit: Edit::JsonRemoveKey {
            path: vec!["skills".into(), name.into()],
        },
    }
}

pub(super) fn write_lock(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

pub(super) fn by_name<'a>(augs: &'a [Installed], name: &str) -> &'a Installed {
    augs.iter().find(|a| a.name == name).unwrap()
}
