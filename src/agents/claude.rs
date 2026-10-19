use std::path::PathBuf;

use super::AgentAdapter;
use crate::env::Env;
use crate::model::{Agent, Augmentation, Scope};

pub struct Claude;

impl AgentAdapter for Claude {
    fn agent(&self) -> Agent {
        Agent::Claude
    }

    fn skill_dirs(&self, env: &Env, scope: Scope) -> Vec<PathBuf> {
        let _ = (env, scope);
        todo!("lane: claude")
    }

    fn hook_files(&self, env: &Env, scope: Scope) -> Vec<PathBuf> {
        let _ = (env, scope);
        todo!("lane: claude")
    }

    fn discover(&self, env: &Env, scope: Scope) -> anyhow::Result<Vec<Augmentation>> {
        let _ = (env, scope);
        todo!("lane: claude")
    }
}
