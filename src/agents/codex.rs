use std::path::PathBuf;

use super::AgentAdapter;
use crate::env::Env;
use crate::model::{Agent, Augmentation, Scope};

pub struct Codex;

impl AgentAdapter for Codex {
    fn agent(&self) -> Agent {
        Agent::Codex
    }

    fn skill_dirs(&self, env: &Env, scope: Scope) -> Vec<PathBuf> {
        let _ = (env, scope);
        todo!("lane: codex-cursor")
    }

    fn discover(&self, env: &Env, scope: Scope) -> anyhow::Result<Vec<Augmentation>> {
        let _ = (env, scope);
        todo!("lane: codex-cursor")
    }
}
