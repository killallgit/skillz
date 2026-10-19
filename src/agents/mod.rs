//! One adapter per agent. Adding an agent = a new module implementing `AgentAdapter` plus a
//! line in `registry()`.

pub mod claude;
pub mod codex;
pub mod cursor;

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use crate::env::Env;
use crate::hooks;
use crate::model::{Agent, Augmentation, Kind, Scope};
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
    join_marketplace_groups(&mut inv.items);
    inv
}

/// npx skills group by their source repo; when a marketplace comes from that same repo, show
/// them under the marketplace instead of as a second group.
fn join_marketplace_groups(items: &mut [Augmentation]) {
    let by_source: BTreeMap<String, String> = items
        .iter()
        .filter(|i| i.kind == Kind::Marketplace)
        .filter_map(|i| Some((i.source.clone()?, i.group.clone())))
        .collect();
    for item in items.iter_mut() {
        if let Some(group) = by_source.get(&item.group) {
            item.group = group.clone();
        }
    }
}

impl Inventory {
    fn absorb(&mut self, source: &str, found: anyhow::Result<Vec<Augmentation>>) {
        match found {
            Ok(items) => self.items.extend(items),
            Err(err) => self.warnings.push(format!("{source}: {err:#}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, kind: Kind, group: &str, source: Option<&str>) -> Augmentation {
        Augmentation {
            id: format!("{kind}:{name}"),
            name: name.into(),
            kind,
            agents: vec![Agent::Codex],
            scope: Scope::Global,
            group: group.into(),
            source: source.map(Into::into),
            children: vec![],
            removal: vec![],
        }
    }

    #[test]
    fn items_from_a_marketplace_repo_join_its_group() {
        let mut items = vec![
            item("mp", Kind::Marketplace, "mp", Some("o/r")),
            item("tdd", Kind::Skill, "o/r", Some("o/r")),
            item("other", Kind::Skill, "x/y", Some("x/y")),
            item("loose", Kind::Skill, "local", None),
        ];
        join_marketplace_groups(&mut items);
        let groups: Vec<&str> = items.iter().map(|i| i.group.as_str()).collect();
        assert_eq!(groups, ["mp", "mp", "x/y", "local"]);
    }
}
