//! The rows of an installed tab's outline: a group per source package, holding the skills,
//! plugins and subagents it provides, both those the scope has and those only the library offers.
//! An item's state says which.

mod available;
mod installed;

use crate::library::{Catalog, ItemKind};
use crate::model::Augmentation;
use crate::package::{Package, group_by};
use crate::profile::Entry;
use crate::tui::widgets::outline::Shape;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum State {
    /// Installed in the scope; the id of the item to remove.
    Installed(String),
    /// Not in the scope; the entry that installs it.
    Available(Entry),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Item {
    pub label: String,
    pub kind: ItemKind,
    /// The agents that have an installed item, shown beside its name.
    pub agents: String,
    /// Where an installed item lives, shown after its agents.
    pub detail: String,
    pub state: State,
    /// Installed globally as well, which matters to a project.
    pub global: bool,
    /// What a plugin brings along, shown beneath it.
    pub owned: Vec<Owned>,
}

/// A skill or hook a plugin provides: shown under the plugin, never acted on by itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Owned {
    pub label: String,
    pub kind: ItemKind,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Group {
    pub package: Package,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Row {
    Package(Group),
    Item(Item),
    Owned(Owned),
}

/// What the rows are built from: what the scope has installed, what the user has installed
/// globally when that is worth pointing out, and the library.
pub(super) struct Sources<'a> {
    pub here: &'a [Augmentation],
    pub also_global: &'a [Augmentation],
    pub library: &'a Catalog,
}

pub(super) fn build(sources: &Sources) -> Vec<Row> {
    let mut items = installed::items(sources);
    items.extend(available::items(sources));
    groups(items)
}

fn groups(items: Vec<(Package, Item)>) -> Vec<Row> {
    group_by(items, |(package, _)| package.clone())
        .into_iter()
        .flat_map(|(package, members)| {
            let mut items: Vec<Item> = members.into_iter().map(|(_, item)| item).collect();
            items.sort_by_key(|item| {
                let available = matches!(item.state, State::Available(_));
                (item.kind, available, item.label.to_lowercase())
            });
            let group = Group { package };
            std::iter::once(Row::Package(group)).chain(item_rows(items))
        })
        .collect()
}

fn item_rows(items: Vec<Item>) -> impl Iterator<Item = Row> {
    items.into_iter().flat_map(|mut item| {
        let owned = std::mem::take(&mut item.owned);
        std::iter::once(Row::Item(item)).chain(owned.into_iter().map(Row::Owned))
    })
}

/// The outline's shape: available groups hang under their header, items under their group and
/// what a plugin provides under the plugin.
pub(super) fn shape(rows: &[Row]) -> Vec<Shape> {
    let (mut group, mut plugin) = (None, None);
    rows.iter()
        .enumerate()
        .map(|(id, row)| match row {
            Row::Package(Group { package }) => {
                group = Some(id);
                (None, package.title())
            }
            Row::Item(item) => {
                plugin = Some(id);
                (group, item.label.clone())
            }
            Row::Owned(owned) => (plugin, owned.label.clone()),
        })
        .collect()
}

/// The items that follow the group at `at`, up to the next group or header.
pub(super) fn items_of(rows: &[Row], at: usize) -> impl Iterator<Item = &Item> {
    rows[at + 1..]
        .iter()
        .take_while(|row| matches!(row, Row::Item(_) | Row::Owned(_)))
        .filter_map(|row| match row {
            Row::Item(item) => Some(item),
            _ => None,
        })
}

#[cfg(test)]
pub(super) mod tests;
