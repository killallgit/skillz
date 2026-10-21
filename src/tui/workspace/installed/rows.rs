//! The rows of an installed tab's outline: a section per kind for what the scope has, then one
//! `Available` group with a section per kind for what only the library offers.

use super::location::location;
use super::presence::{has_counterpart, has_library_item};
use crate::library::{Catalog, ItemKind, LibraryItem};
use crate::model::Augmentation;
use crate::profile::Entry;
use crate::tui::widgets::outline::Shape;

/// The sections in the order they are shown.
const SECTIONS: [ItemKind; 5] = [
    ItemKind::Skill,
    ItemKind::Plugin,
    ItemKind::Mcp,
    ItemKind::Hook,
    ItemKind::Subagent,
];

const AVAILABLE: &str = "Available";

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
    /// Where an installed item lives, shown beside its name.
    pub detail: String,
    pub state: State,
    /// Installed globally as well, which matters to a project.
    pub global: bool,
}

/// Whether a section lists what the scope has or what the library could add.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Part {
    Installed,
    Available,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Row {
    /// Heads the sections of what is not installed.
    Available {
        total: usize,
    },
    Section {
        part: Part,
        kind: ItemKind,
        total: usize,
    },
    Item(Item),
}

/// What the rows are built from: what the scope has installed, what the user has installed
/// globally when that is worth pointing out, and the library.
pub(super) struct Sources<'a> {
    pub here: &'a [Augmentation],
    pub also_global: &'a [Augmentation],
    pub library: &'a Catalog,
}

/// The installed sections, then the available ones under their header; a part with nothing in
/// it is left out.
pub(super) fn build(sources: &Sources) -> Vec<Row> {
    let installed = sections(Part::Installed, sources);
    let available = sections(Part::Available, sources);
    let offered = available
        .iter()
        .filter(|row| matches!(row, Row::Item(_)))
        .count();
    let header = (offered > 0).then_some(Row::Available { total: offered });
    installed
        .into_iter()
        .chain(header)
        .chain(available)
        .collect()
}

fn sections(part: Part, sources: &Sources) -> Vec<Row> {
    SECTIONS
        .into_iter()
        .map(|kind| (kind, items_of(part, kind, sources)))
        .filter(|(_, items)| !items.is_empty())
        .flat_map(|(kind, items)| {
            let header = Row::Section {
                part,
                kind,
                total: items.len(),
            };
            std::iter::once(header).chain(items.into_iter().map(Row::Item))
        })
        .collect()
}

fn items_of(part: Part, kind: ItemKind, sources: &Sources) -> Vec<Item> {
    let mut items = match part {
        Part::Installed => installed_items(kind, sources),
        Part::Available => available_items(kind, sources),
    };
    items.sort_by_key(|item| item.label.to_lowercase());
    items
}

fn installed_items(kind: ItemKind, sources: &Sources) -> Vec<Item> {
    sources
        .here
        .iter()
        .filter(|a| ItemKind::of(a.kind) == Some(kind))
        .map(|item| Item {
            label: item.name.clone(),
            detail: location(item),
            state: State::Installed(item.id.clone()),
            global: has_counterpart(sources.also_global, item),
        })
        .collect()
}

fn available_items(kind: ItemKind, sources: &Sources) -> Vec<Item> {
    sources
        .library
        .items()
        .iter()
        .filter(|item| item.kind() == kind)
        .filter(|item| !has_library_item(sources.here, item))
        .map(|item| available_item(item, sources))
        .collect()
}

fn available_item(item: &LibraryItem, sources: &Sources) -> Item {
    Item {
        label: item.label.clone(),
        detail: String::new(),
        state: State::Available(item.entry.clone()),
        global: has_library_item(sources.also_global, item),
    }
}

/// The outline's shape: available sections hang under their header, items under their section.
pub(super) fn shape(rows: &[Row]) -> Vec<Shape> {
    let (mut header, mut section) = (None, None);
    rows.iter()
        .enumerate()
        .map(|(id, row)| match row {
            Row::Available { .. } => {
                header = Some(id);
                (None, AVAILABLE.to_string())
            }
            Row::Section {
                part: Part::Installed,
                kind,
                ..
            } => {
                section = Some(id);
                (None, kind.title().to_string())
            }
            Row::Section { kind, .. } => {
                section = Some(id);
                (header, format!("{AVAILABLE} {}", kind.title()))
            }
            Row::Item(item) => (section, item.label.clone()),
        })
        .collect()
}

#[cfg(test)]
pub(super) mod tests;
