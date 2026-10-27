//! The rows of the plugins list: a package each, with the installed items that make it present
//! in the project and the library entries that would put it there.

use std::collections::HashSet;

use super::info;
use crate::app::library::{LibraryItem, now_secs};
use crate::app::package::{Package, group_by};
use crate::app::profile::{Key, installed_keys, keys};
use crate::app::workspace::Model;
use crate::tui::widgets::outline::Shape;
use skillz_harness::Spec;
use skillz_harness::model::{Agent, Installed, Kind, Scope};

/// A package of plugins, skills or subagents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Plugin {
    pub package: Package,
    /// Ids of the installed items from this package; empty when it is not installed.
    pub installed: Vec<String>,
    /// What the library installs for this package.
    pub entries: Vec<Spec>,
    /// The tools that have it installed.
    pub tools: Vec<Agent>,
    /// What the info modal says about it.
    pub info: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Row {
    /// A package the list's scope can have or lacks: the one the cursor can flip.
    Own(Plugin),
    /// A blank line above the divider, when there are project packages to set it apart from, and
    /// below it.
    Gap,
    /// The line between the project's packages and the user's, which come after it.
    Divider,
    /// A package installed at user level: shown, never changed from here.
    Global(Plugin),
}

impl Row {
    pub fn name(&self) -> String {
        match self {
            Row::Own(plugin) | Row::Global(plugin) => plugin.name(),
            Row::Gap => String::new(),
            Row::Divider => DIVIDER.into(),
        }
    }

    pub fn shape(&self) -> Shape {
        (None, self.name())
    }

    /// The gap and the divider are drawn in the list but the cursor does not rest on them.
    pub fn is_inert(&self) -> bool {
        matches!(self, Row::Gap | Row::Divider)
    }
}

pub(super) const DIVIDER: &str = "global";

impl Plugin {
    pub fn is_installed(&self) -> bool {
        !self.installed.is_empty()
    }

    pub fn name(&self) -> String {
        self.package.title()
    }

    fn take(&mut self, items: &[&Installed]) {
        for item in items {
            if !self.installed.contains(&item.id) {
                self.installed.push(item.id.clone());
            }
            for agent in &item.agents {
                if !self.tools.contains(agent) {
                    self.tools.push(*agent);
                }
            }
        }
    }
}

/// Skills, plugins and subagents come from a package; MCP servers, hooks and hand-made files
/// are not part of this list.
fn listed(package: &Package) -> bool {
    matches!(package, Package::Source(_) | Package::Marketplace(_))
}

/// The packages of `scope` by name. A package the library does not know still shows while it is
/// installed, so it can be removed. For the project, the user's packages follow below a divider:
/// one the user has installed and the project has not is already in effect, so it is listed only
/// there. User level has no such section: it is the whole list.
pub(super) fn build(model: &Model, scope: Scope) -> Vec<Row> {
    let items: Vec<&LibraryItem> = model.library.items().iter().collect();
    let now = now_secs();
    match scope {
        Scope::Project => project_and_user(&items, model, now),
        Scope::Global => packages(&items, &model.global, now)
            .into_iter()
            .map(Row::Own)
            .collect(),
    }
}

fn project_and_user(items: &[&LibraryItem], model: &Model, now: u64) -> Vec<Row> {
    let user: Vec<Plugin> = packages(items, &model.global, now)
        .into_iter()
        .filter(Plugin::is_installed)
        .collect();
    let mut rows: Vec<Row> = packages(items, &model.installed, now)
        .into_iter()
        .filter(|plugin| {
            plugin.is_installed() || !user.iter().any(|held| held.package == plugin.package)
        })
        .map(Row::Own)
        .collect();
    if !user.is_empty() {
        if !rows.is_empty() {
            rows.push(Row::Gap);
        }
        rows.push(Row::Divider);
        rows.push(Row::Gap);
        rows.extend(user.into_iter().map(Row::Global));
    }
    rows
}

/// A plugin per package the library offers or `installed` holds, by name.
fn packages(items: &[&LibraryItem], installed: &[Installed], now: u64) -> Vec<Plugin> {
    let have: Vec<(&Installed, HashSet<Key>)> = installed
        .iter()
        .map(|item| (item, item.parts().iter().flat_map(installed_keys).collect()))
        .collect();
    let mut rows: Vec<Plugin> = group_by(items.to_vec(), |item| Package::of_entry(&item.entry))
        .into_iter()
        .filter(|(package, _)| listed(package))
        .map(|(package, offered)| {
            let wanted: HashSet<Key> = offered.iter().flat_map(|item| keys(&item.entry)).collect();
            let held: Vec<&Installed> = have
                .iter()
                .filter(|(_, keys)| !keys.is_disjoint(&wanted))
                .map(|(item, _)| *item)
                .collect();
            let mut row = Plugin {
                package,
                installed: vec![],
                entries: offered.iter().map(|item| item.entry.clone()).collect(),
                tools: vec![],
                info: vec![],
            };
            row.take(&with_marketplaces(held, installed));
            row
        })
        .collect();
    let claimed: HashSet<&str> = rows
        .iter()
        .flat_map(|row| row.installed.iter().map(String::as_str))
        .collect();
    let rest: Vec<&Installed> = installed
        .iter()
        .filter(|item| !claimed.contains(item.id.as_str()))
        .collect();
    for (package, items) in group_by(rest, |item| Package::of(item)) {
        if !listed(&package) {
            continue;
        }
        match rows.iter_mut().find(|row| row.package == package) {
            Some(row) => row.take(&items),
            None => {
                let mut row = Plugin {
                    package,
                    installed: vec![],
                    entries: vec![],
                    tools: vec![],
                    info: vec![],
                };
                row.take(&items);
                rows.push(row);
            }
        }
    }
    for row in &mut rows {
        let held: Vec<&Installed> = installed
            .iter()
            .filter(|item| row.installed.contains(&item.id))
            .collect();
        let offered: Vec<&LibraryItem> = items
            .iter()
            .copied()
            .filter(|item| Package::of_entry(&item.entry) == row.package)
            .collect();
        row.info = info::lines(row, &offered, &held, now);
    }
    rows.sort_by_key(|row| row.name().to_lowercase());
    rows
}

/// `held` with the marketplaces whose installed plugins `held` has all of: removing a marketplace
/// removes every plugin installed from it, so it goes only with the last of them. It holds no key of
/// its own to match an entry by.
fn with_marketplaces<'a>(mut held: Vec<&'a Installed>, all: &'a [Installed]) -> Vec<&'a Installed> {
    let held_ids: HashSet<&str> = held.iter().map(|item| item.id.as_str()).collect();
    let remaining: HashSet<&str> = all
        .iter()
        .filter(|item| item.kind == Kind::Plugin && !held_ids.contains(item.id.as_str()))
        .map(|item| item.group.as_str())
        .collect();
    let groups: HashSet<&str> = held
        .iter()
        .filter(|item| item.kind == Kind::Plugin)
        .map(|item| item.group.as_str())
        .filter(|group| !remaining.contains(group))
        .collect();
    held.extend(
        all.iter()
            .filter(|item| item.kind == Kind::Marketplace && groups.contains(item.group.as_str())),
    );
    held
}
