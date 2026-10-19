use std::path::{Path, PathBuf};

use crate::env::Env;
use crate::model::{Agent, Augmentation, Scope};

/// A directory an agent loads skills from (each skill is a subdir with `SKILL.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillDir {
    pub agent: Agent,
    pub scope: Scope,
    pub path: PathBuf,
}

/// A skill found in a source tree (used by install to offer choices).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailableSkill {
    pub name: String,
    pub path: PathBuf,
}

/// Installed skills across all `dirs`, deduplicated by canonical path.
pub fn discover(env: &Env, dirs: &[SkillDir]) -> anyhow::Result<Vec<Augmentation>> {
    let _ = (env, dirs);
    todo!("lane: skills")
}

/// Every skill (dir containing `SKILL.md`) under `root`, recursively, sorted by name.
pub fn scan_available(root: &Path) -> anyhow::Result<Vec<AvailableSkill>> {
    let _ = root;
    todo!("lane: skills")
}
