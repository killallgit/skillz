//! The rows of an installed tab's outline: a section per kind of item the scope has, and in it
//! the units that can be installed or removed whole. Skills hang under the package they came
//! from; a plugin is one row that says what it provides, and what it provides is never listed a
//! second time among the skills and hooks.

mod installed;

use crate::library::ItemKind;
use crate::package::{Package, group_by};
use crate::tui::widgets::outline::Shape;
use skillz_harness::model::Installed;

/// The sections in the order they are shown.
const SECTIONS: [ItemKind; 5] = [
    ItemKind::Plugin,
    ItemKind::Skill,
    ItemKind::Subagent,
    ItemKind::Mcp,
    ItemKind::Hook,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Item {
    pub label: String,
    pub kind: ItemKind,
    /// The agents that have the item, shown beside its name.
    pub agents: String,
    /// Where the item lives, or what a plugin provides; shown after its agents.
    pub detail: String,
    /// The id to remove or update it by.
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Group {
    pub package: Package,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Row {
    /// Heads the items of one kind.
    Section(ItemKind),
    /// The skills of one source, under the skills section.
    Package(Group),
    Item(Item),
}

pub(super) fn build(here: &[Installed]) -> Vec<Row> {
    let items = installed::items(here);
    SECTIONS
        .into_iter()
        .flat_map(|kind| {
            let of_kind: Vec<(Package, Item)> = items
                .iter()
                .filter(|(_, item)| item.kind == kind)
                .cloned()
                .collect();
            let rows = match kind {
                ItemKind::Skill => grouped(of_kind),
                _ => listed(of_kind),
            };
            (!rows.is_empty())
                .then_some(Row::Section(kind))
                .into_iter()
                .chain(rows)
        })
        .collect()
}

fn listed(items: Vec<(Package, Item)>) -> Vec<Row> {
    let mut items: Vec<Item> = items.into_iter().map(|(_, item)| item).collect();
    items.sort_by_key(|item| item.label.to_lowercase());
    items.into_iter().map(Row::Item).collect()
}

fn grouped(items: Vec<(Package, Item)>) -> Vec<Row> {
    group_by(items, |(package, _)| package.clone())
        .into_iter()
        .flat_map(|(package, members)| {
            let mut items: Vec<Item> = members.into_iter().map(|(_, item)| item).collect();
            items.sort_by_key(|item| item.label.to_lowercase());
            std::iter::once(Row::Package(Group { package })).chain(items.into_iter().map(Row::Item))
        })
        .collect()
}

/// The outline's shape: packages hang under the skills section, items under their package or,
/// where there is none, their section.
pub(super) fn shape(rows: &[Row]) -> Vec<Shape> {
    let (mut section, mut group) = (None, None);
    rows.iter()
        .enumerate()
        .map(|(id, row)| match row {
            Row::Section(kind) => {
                section = Some(id);
                group = None;
                (None, kind.plural().to_string())
            }
            Row::Package(Group { package }) => {
                group = Some(id);
                (section, package.title())
            }
            Row::Item(item) => (group.or(section), item.label.clone()),
        })
        .collect()
}

/// The items under the section or package at `at`; an item stands for itself alone.
pub(super) fn items_of(rows: &[Row], at: usize) -> impl Iterator<Item = &Item> {
    let ends_at_a_package = matches!(rows[at], Row::Package(_));
    rows[at + 1..]
        .iter()
        .take_while(move |row| match row {
            Row::Section(_) => false,
            Row::Package(_) => !ends_at_a_package,
            Row::Item(_) => true,
        })
        .filter_map(|row| match row {
            Row::Item(item) => Some(item),
            _ => None,
        })
}

#[cfg(test)]
pub(super) mod tests;
