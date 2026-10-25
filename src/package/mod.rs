//! The package an item comes from, as `npx skills list` groups them: an `owner/repo`, a plugin
//! marketplace, or `local files` for hand-made items. Things that come from no package (MCP
//! servers, hooks no plugin declares) keep a group of their own. Pure; every view groups by it.

#[cfg(test)]
mod tests;

use std::path::Path;

use crate::stale::local_source;
use skillz_harness::Spec;
use skillz_harness::hooks::UNOWNED_HOOKS;
use skillz_harness::model::{Installed, Kind};

const LOCAL_FILES: &str = "local files";
const MCP_SERVERS: &str = "MCP servers";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Package {
    /// An `owner/repo`, repository URL or path that skills or subagents were fetched from.
    Source(String),
    Marketplace(String),
    Local,
    Mcp,
    Hooks,
}

impl Package {
    pub fn of(item: &Installed) -> Package {
        match (item.kind, &item.source) {
            (Kind::Mcp, _) => Package::Mcp,
            (Kind::Hook, _) => Package::Hooks,
            (Kind::Marketplace | Kind::Plugin, _) => Package::Marketplace(item.group.clone()),
            (Kind::Skill | Kind::Subagent, Some(source)) => Package::Source(source.clone()),
            (Kind::Skill | Kind::Subagent, None) => Package::Local,
        }
    }

    pub fn of_entry(entry: &Spec) -> Package {
        match entry {
            Spec::Skill { .. } | Spec::Subagent { .. } if local_source(entry).is_some() => {
                Package::Local
            }
            Spec::Skill { source, .. } | Spec::Subagent { source, .. } => {
                Package::Source(source.clone())
            }
            Spec::Plugin {
                source,
                marketplace,
                ..
            } if Path::new(source).is_absolute() => Package::Marketplace(marketplace.clone()),
            // A marketplace hosted in a repo sits with that repo's skills.
            Spec::Plugin { source, .. } => Package::Source(source.clone()),
            Spec::Mcp { .. } => Package::Mcp,
            Spec::Hook { .. } => Package::Hooks,
        }
    }

    pub fn title(&self) -> String {
        match self {
            Package::Source(name) | Package::Marketplace(name) => name.clone(),
            Package::Local => LOCAL_FILES.into(),
            Package::Mcp => MCP_SERVERS.into(),
            Package::Hooks => UNOWNED_HOOKS.into(),
        }
    }

    pub fn is_marketplace(&self) -> bool {
        matches!(self, Package::Marketplace(_))
    }

    /// Named packages alphabetically, then local files, MCP servers and unowned hooks.
    fn order(&self) -> (u8, String) {
        match self {
            Package::Source(name) | Package::Marketplace(name) => (0, name.to_lowercase()),
            Package::Local => (1, String::new()),
            Package::Mcp => (2, String::new()),
            Package::Hooks => (3, String::new()),
        }
    }
}

/// `items` split by package in display order; within a package they keep their order.
pub fn group_by<T>(items: Vec<T>, package: impl Fn(&T) -> Package) -> Vec<(Package, Vec<T>)> {
    let mut groups: Vec<(Package, Vec<T>)> = Vec::new();
    for item in items {
        let wanted = package(&item);
        match groups.iter_mut().find(|(have, _)| *have == wanted) {
            Some((_, members)) => members.push(item),
            None => groups.push((wanted, vec![item])),
        }
    }
    groups.sort_by_key(|(package, _)| package.order());
    groups
}
