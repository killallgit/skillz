//! Everything installed, gathered from every adapter plus the shared skill and hook discovery.

use std::collections::{BTreeMap, HashSet};

use super::AgentAdapter;
use crate::env::Env;
use crate::hooks;
use crate::model::{Augmentation, Kind, Scope};
use crate::skills::{self, SkillDir};

/// Everything installed, plus warnings for sources that failed to read.
#[derive(Debug, Default)]
pub struct Inventory {
    pub items: Vec<Augmentation>,
    pub warnings: Vec<String>,
}

impl Inventory {
    fn absorb(&mut self, source: &str, found: anyhow::Result<Vec<Augmentation>>) {
        match found {
            Ok(items) => self.items.extend(items),
            Err(err) => self.warnings.push(format!("{source}: {err:#}")),
        }
    }
}

/// Combine every adapter's discovery and the shared skill discovery for `scopes`.
/// Items are deduplicated by id; a failing source becomes a warning, not an error.
pub fn inventory(adapters: &[Box<dyn AgentAdapter>], env: &Env, scopes: &[Scope]) -> Inventory {
    let mut inv = Inventory::default();
    // One discover over every scope, so a project link into a global skill is its own item.
    inv.absorb(
        "skills",
        skills::discover(env, &skill_dirs(adapters, env, scopes)),
    );
    for &scope in scopes {
        for a in adapters {
            absorb_adapter(&mut inv, a.as_ref(), env, scope);
        }
    }
    let mut seen = HashSet::new();
    inv.items.retain(|item| seen.insert(item.id.clone()));
    join_marketplace_groups(&mut inv.items);
    inv
}

fn skill_dirs(adapters: &[Box<dyn AgentAdapter>], env: &Env, scopes: &[Scope]) -> Vec<SkillDir> {
    scopes
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
        .collect()
}

fn absorb_adapter(inv: &mut Inventory, a: &dyn AgentAdapter, env: &Env, scope: Scope) {
    for file in a.hook_files(env, scope) {
        inv.absorb(
            &file.display().to_string(),
            hooks::discover(&file, a.agent(), scope),
        );
    }
    inv.absorb(&a.agent().to_string(), a.discover(env, scope));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Agent, Installer};

    fn item(name: &str, kind: Kind, group: &str, source: Option<&str>) -> Augmentation {
        Augmentation {
            hooks: Vec::new(),
            id: format!("{kind}:{name}"),
            name: name.into(),
            kind,
            agents: vec![Agent::Codex],
            scope: Scope::Global,
            group: group.into(),
            source: source.map(Into::into),
            installer: Installer::Local,
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
