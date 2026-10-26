//! The rows of an installed tab's outline: a section per kind of item the scope has, and in it
//! the units that can be installed or removed whole. Skills hang under the package they came
//! from; a plugin is one row that says what it provides, and what it provides is never listed a
//! second time among the skills and hooks.

mod installed;

use crate::app::package::{Package, group_by};
use crate::tui::widgets::outline::Shape;
use skillz_harness::model::{Installed, Kind};

/// The sections in the order they are shown.
const SECTIONS: [Kind; 6] = [
    Kind::Marketplace,
    Kind::Plugin,
    Kind::Skill,
    Kind::Subagent,
    Kind::Mcp,
    Kind::Hook,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Item {
    pub label: String,
    pub kind: Kind,
    /// The agents that have the item, shown beside its name.
    pub agents: String,
    /// Where the item lives, or what a plugin provides; shown after its agents.
    pub detail: String,
    /// The id to remove or update it by.
    pub id: String,
    /// Display-only plugin contents; effects always belong to the whole plugin.
    pub children: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Group {
    pub package: Package,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Row {
    Inherited,
    /// Heads the items of one kind.
    Section(Kind),
    /// The skills of one source, under the skills section.
    Package(Group),
    Item(Item),
    Detail(String),
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
                Kind::Skill => grouped(of_kind),
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
    items.into_iter().flat_map(item_rows).collect()
}

fn item_rows(item: Item) -> impl Iterator<Item = Row> {
    let children = item.children.clone().into_iter().map(Row::Detail);
    std::iter::once(Row::Item(item)).chain(children)
}

fn grouped(items: Vec<(Package, Item)>) -> Vec<Row> {
    group_by(items, |(package, _)| package.clone())
        .into_iter()
        .flat_map(|(package, members)| {
            let mut items: Vec<Item> = members.into_iter().map(|(_, item)| item).collect();
            items.sort_by_key(|item| item.label.to_lowercase());
            std::iter::once(Row::Package(Group { package }))
                .chain(items.into_iter().flat_map(item_rows))
        })
        .collect()
}

/// The outline's shape: packages hang under the skills section, items under their package or,
/// where there is none, their section.
pub(super) fn shape(rows: &[Row]) -> Vec<Shape> {
    let (mut inherited, mut section, mut group, mut item) = (None, None, None, None);
    rows.iter()
        .enumerate()
        .map(|(id, row)| match row {
            Row::Inherited => {
                inherited = Some(id);
                section = None;
                group = None;
                (None, "Global installs · read only".into())
            }
            Row::Section(kind) => {
                section = Some(id);
                group = None;
                (inherited, plural(*kind).to_string())
            }
            Row::Package(Group { package }) => {
                group = Some(id);
                (section, package.title())
            }
            Row::Item(value) => {
                item = Some(id);
                (group.or(section), value.label.clone())
            }
            Row::Detail(label) => (item, label.clone()),
        })
        .collect()
}

/// The items under the section or package at `at`; an item stands for itself alone.
pub(super) fn items_of(rows: &[Row], at: usize) -> impl Iterator<Item = &Item> {
    let ends_at_a_package = matches!(rows[at], Row::Package(_));
    rows[at + 1..]
        .iter()
        .take_while(move |row| match row {
            Row::Inherited | Row::Section(_) => false,
            Row::Package(_) => !ends_at_a_package,
            Row::Item(_) | Row::Detail(_) => true,
        })
        .filter_map(|row| match row {
            Row::Item(item) => Some(item),
            _ => None,
        })
}

#[cfg(test)]
pub(super) mod tests;

/// Names for native installed kinds, including marketplace registrations.
pub(super) fn plural(kind: Kind) -> &'static str {
    match kind {
        Kind::Marketplace => "marketplaces",
        Kind::Plugin => "plugins",
        Kind::Skill => "skills",
        Kind::Subagent => "agents",
        Kind::Mcp => "MCP servers",
        Kind::Hook => "hooks",
    }
}
