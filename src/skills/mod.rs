mod discover;
mod entry;
mod local;
mod ownership;
mod removal;

#[cfg(test)]
mod testutil;

use std::path::PathBuf;

use crate::model::{Agent, Scope};

pub use discover::discover;

/// A directory an agent loads skills from (each skill is a subdir with `SKILL.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillDir {
    pub agent: Agent,
    pub scope: Scope,
    pub path: PathBuf,
}
