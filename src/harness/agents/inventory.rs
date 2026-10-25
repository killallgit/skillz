//! Everything installed, gathered from every adapter plus the shared skill and hook discovery.

use std::collections::{BTreeMap, HashSet};

use super::Harness;
use crate::harness::env::Env;
use crate::harness::hooks::{self, GroupHints};
use crate::harness::model::{Installed, Kind, Scope};
use crate::harness::npx;

/// Everything installed, plus warnings for sources that failed to read.
#[derive(Debug, Default)]
pub struct Inventory {
    pub items: Vec<Installed>,
    pub warnings: Vec<String>,
}

impl Inventory {
    /// The items, or an error naming every source that failed to read: for callers that act on
    /// what is installed and must not guess from a partial scan.
    pub fn complete(self) -> anyhow::Result<Vec<Installed>> {
        if self.warnings.is_empty() {
            return Ok(self.items);
        }
        anyhow::bail!(
            "cannot read what is installed: {}",
            self.warnings.join("; ")
        )
    }

    fn absorb(&mut self, source: &str, found: anyhow::Result<Vec<Installed>>) {
        match found {
            Ok(items) => self.items.extend(items),
            Err(err) => self.warnings.push(format!("{source}: {err:#}")),
        }
    }
}

/// Combine every adapter's discovery and the shared skill discovery for `scopes`.
/// Items are deduplicated by id; a failing source becomes a warning, not an error.
pub fn inventory(adapters: &[Box<dyn Harness>], env: &Env, scopes: &[Scope]) -> Inventory {
    let mut inv = Inventory::default();
    // One discover over every scope, so a project link into a global skill is its own item.
    let agents: Vec<_> = adapters.iter().map(|a| a.agent()).collect();
    inv.absorb("skills", npx::installed(env, &agents, scopes));
    let hints = group_hints(adapters);
    for &scope in scopes {
        for a in adapters {
            absorb_adapter(&mut inv, a.as_ref(), (env, scope), &hints);
        }
    }
    let mut seen = HashSet::new();
    inv.items.retain(|item| seen.insert(item.id.clone()));
    join_marketplace_groups(&mut inv.items);
    inv
}

fn group_hints(adapters: &[Box<dyn Harness>]) -> GroupHints {
    GroupHints {
        plugin_root_vars: adapters
            .iter()
            .filter_map(|a| a.plugin_root_var())
            .collect(),
        agent_dirs: adapters
            .iter()
            .flat_map(|a| a.home_dir_names())
            .copied()
            .collect(),
    }
}

fn absorb_adapter(
    inv: &mut Inventory,
    a: &dyn Harness,
    (env, scope): (&Env, Scope),
    hints: &GroupHints,
) {
    for file in a.hook_files(env, scope) {
        inv.absorb(
            &file.display().to_string(),
            hooks::discover(&file, a.agent(), scope, hints),
        );
    }
    inv.absorb(&a.agent().to_string(), a.installed(env, scope));
}

/// npx skills group by their source repo; when a marketplace comes from that same repo, show
/// them under the marketplace instead of as a second group.
fn join_marketplace_groups(items: &mut [Installed]) {
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
    use crate::harness::model::{Agent, Installer};

    fn item(name: &str, kind: Kind, group: &str, source: Option<&str>) -> Installed {
        Installed {
            details: Default::default(),
            hooks: Vec::new(),
            members: Vec::new(),
            id: format!("{kind}:{name}"),
            name: name.into(),
            kind,
            agents: vec![Agent::Codex],
            scope: Scope::Global,
            group: group.into(),
            source: source.map(Into::into),
            installer: Installer::Local,
            origin: Default::default(),
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
    #[test]
    fn invalid_discovery_directories_make_the_inventory_incomplete() {
        for directory in [".claude/skills", ".claude/agents"] {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join(directory);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "not a directory").unwrap();
            let env = Env::new(temp.path().to_path_buf(), temp.path().join("project"));
            let adapters = super::super::registry();

            let found = inventory(&adapters, &env, &[Scope::Global]);

            assert!(!found.warnings.is_empty());
            assert!(found.complete().is_err());
        }
    }
}
