//! One adapter per agent. Adding an agent = a new module implementing `AgentAdapter` plus a
//! line in `registry()`.

pub mod claude;
pub mod codex;
pub mod cursor;

use std::collections::HashSet;
use std::path::PathBuf;

use crate::env::Env;
use crate::hooks;
use crate::model::{Agent, Augmentation, Scope};
use crate::skills::{self, SkillDir};

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
    let mut inv = Inventory::default();
    // One discover over every scope, so a project link into a global skill is its own item.
    let dirs: Vec<SkillDir> = scopes
        .iter()
        .flat_map(|&scope| {
            adapters.iter().flat_map(move |a| {
                a.skill_dirs(env, scope)
                    .into_iter()
                    .map(move |path| SkillDir {
                        agent: a.agent(),
                        scope,
                        path,
                    })
            })
        })
        .collect();
    inv.absorb("skills", skills::discover(env, &dirs));
    for &scope in scopes {
        for a in adapters {
            for file in a.hook_files(env, scope) {
                inv.absorb(
                    &file.display().to_string(),
                    hooks::discover(&file, a.agent(), scope),
                );
            }
            inv.absorb(&a.agent().to_string(), a.discover(env, scope));
        }
    }
    let mut seen = HashSet::new();
    inv.items.retain(|item| seen.insert(item.id.clone()));
    inv
}

impl Inventory {
    fn absorb(&mut self, source: &str, found: anyhow::Result<Vec<Augmentation>>) {
        match found {
            Ok(items) => self.items.extend(items),
            Err(err) => self.warnings.push(format!("{source}: {err:#}")),
        }
    }
}
