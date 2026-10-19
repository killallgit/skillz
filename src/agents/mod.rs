//! One adapter per agent. Adding an agent = a new module implementing `AgentAdapter` plus a
//! line in `registry()`.

pub mod claude;
pub mod codex;
pub mod cursor;

use std::path::PathBuf;

use crate::env::Env;
use crate::model::{Agent, Augmentation, Scope};

pub trait AgentAdapter {
    fn agent(&self) -> Agent;
    /// Directories this agent loads loose skills from, for `scope`.
    fn skill_dirs(&self, env: &Env, scope: Scope) -> Vec<PathBuf>;
    /// Hook config files this agent reads for `scope` (parsed by `hooks::discover`).
    fn hook_files(&self, env: &Env, scope: Scope) -> Vec<PathBuf>;
    /// Plugins and marketplaces for `scope`. Loose skills and hooks are discovered by the
    /// shared `skills`/`hooks` modules from `skill_dirs`/`hook_files`.
    fn discover(&self, env: &Env, scope: Scope) -> anyhow::Result<Vec<Augmentation>>;
}

pub fn registry() -> Vec<Box<dyn AgentAdapter>> {
    vec![
        Box::new(claude::Claude),
        Box::new(codex::Codex),
        Box::new(cursor::Cursor),
    ]
}

/// Everything installed, plus warnings for sources that failed to read.
#[derive(Debug, Default)]
pub struct Inventory {
    pub items: Vec<Augmentation>,
    pub warnings: Vec<String>,
}

/// Combine every adapter's discovery and the shared skill discovery for `scopes`.
/// Items are deduplicated by id; a failing source becomes a warning, not an error.
pub fn inventory(adapters: &[Box<dyn AgentAdapter>], env: &Env, scopes: &[Scope]) -> Inventory {
    let _ = (adapters, env, scopes);
    todo!("lane: cli")
}
