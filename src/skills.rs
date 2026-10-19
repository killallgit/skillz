use std::path::PathBuf;

use crate::env::Env;
use crate::model::{Agent, Augmentation, Scope};

/// A directory an agent loads skills from (each skill is a subdir with `SKILL.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillDir {
    pub agent: Agent,
    pub scope: Scope,
    pub path: PathBuf,
}

/// Installed skills across all `dirs`, deduplicated by canonical path.
pub fn discover(env: &Env, dirs: &[SkillDir]) -> anyhow::Result<Vec<Augmentation>> {
    let _ = (env, dirs);
    todo!("lane: skills")
}
