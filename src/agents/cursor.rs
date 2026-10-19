use std::path::PathBuf;

use super::AgentAdapter;
use crate::env::Env;
use crate::model::{Agent, Augmentation, Scope};

pub struct Cursor;

impl AgentAdapter for Cursor {
    fn agent(&self) -> Agent {
        Agent::Cursor
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
