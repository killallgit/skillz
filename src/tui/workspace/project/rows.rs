//! The rows of the project outline: a group each for what the project has, what the user has
//! installed globally and what only the library offers, each split into a section per kind.

use super::location::location;
use super::presence::{entry_of, has_counterpart, has_library_item};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Group {
    Project,
    Global,
    Available,
}

impl Group {
    const ALL: [Group; 3] = [Group::Project, Group::Global, Group::Available];

    pub fn title(self) -> &'static str {
        match self {
            Group::Project => "Project",
            Group::Global => "Global",
            Group::Available => "Available",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum State {
    /// Installed in the project; the id of the item to remove.
    Installed(String),
    /// Not in the project; the entry that installs it.
    Available(Entry),
    /// Installed globally and nothing this tab can install or remove.
    Held { in_project: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Item {
    pub label: String,
    /// Where an installed item lives, shown beside its name.
    pub detail: String,
    pub state: State,
    pub group: Group,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Row {
    Group {
        group: Group,
        total: usize,
    },
    Section {
        group: Group,
        kind: ItemKind,
        total: usize,
    },
    Item(Item),
}

/// What the rows are built from: what the project and the user have installed, and the library.
pub(super) struct Sources<'a> {
    pub project: &'a [Augmentation],
    pub global: &'a [Augmentation],
    pub library: &'a Catalog,
}

/// Every group that has something, each followed by its sections and their items, in preorder.
pub(super) fn build(sources: &Sources) -> Vec<Row> {
    Group::ALL
        .into_iter()
        .flat_map(|group| group_rows(group, sources))
        .collect()
}

fn group_rows(group: Group, sources: &Sources) -> Vec<Row> {
    let sections: Vec<(ItemKind, Vec<Item>)> = SECTIONS
        .into_iter()
        .map(|kind| (kind, items_of(group, kind, sources)))
        .filter(|(_, items)| !items.is_empty())
        .collect();
    let total = sections.iter().map(|(_, items)| items.len()).sum();
    if total == 0 {
        return vec![];
    }
    let rows = sections.into_iter().flat_map(|(kind, items)| {
        let header = Row::Section {
            group,
            kind,
            total: items.len(),
        };
        std::iter::once(header).chain(items.into_iter().map(Row::Item))
    });
    std::iter::once(Row::Group { group, total })
        .chain(rows)
        .collect()
}

fn items_of(group: Group, kind: ItemKind, sources: &Sources) -> Vec<Item> {
    let mut items = match group {
        Group::Project => installed_items(sources.project, kind, project_item),
        Group::Global => installed_items(sources.global, kind, |item| global_item(item, sources)),
        Group::Available => available_items(kind, sources),
    };
    items.sort_by_key(|item| item.label.to_lowercase());
    items
}

fn installed_items(
    have: &[Augmentation],
    kind: ItemKind,
    make: impl Fn(&Augmentation) -> Item,
) -> Vec<Item> {
    have.iter()
        .filter(|a| ItemKind::of(a.kind) == Some(kind))
        .map(make)
        .collect()
}

fn project_item(item: &Augmentation) -> Item {
    Item {
        label: item.name.clone(),
        detail: location(item),
        state: State::Installed(item.id.clone()),
        group: Group::Project,
    }
}

fn global_item(item: &Augmentation, sources: &Sources) -> Item {
    let in_project = has_counterpart(sources.project, item);
    let state = match (in_project, entry_of(item)) {
        (false, Some(entry)) => State::Available(entry),
        _ => State::Held { in_project },
    };
    Item {
        label: item.name.clone(),
        detail: location(item),
        state,
        group: Group::Global,
    }
}

fn available_items(kind: ItemKind, sources: &Sources) -> Vec<Item> {
    sources
        .library
        .items()
        .iter()
        .filter(|item| item.kind() == kind)
        .filter(|item| !has_library_item(sources.project, item))
        .filter(|item| !has_library_item(sources.global, item))
        .map(available_item)
        .collect()
}

fn available_item(item: &LibraryItem) -> Item {
    Item {
        label: item.label.clone(),
        detail: String::new(),
        state: State::Available(item.entry.clone()),
        group: Group::Available,
    }
}

/// The outline's shape: sections hang under their group, items under their section.
pub(super) fn shape(rows: &[Row]) -> Vec<Shape> {
    let (mut group, mut section) = (None, None);
    rows.iter()
        .enumerate()
        .map(|(id, row)| match row {
            Row::Group { group: g, .. } => {
                group = Some(id);
                (None, g.title().to_string())
            }
            Row::Section { group: g, kind, .. } => {
                section = Some(id);
                (group, format!("{} {}", g.title(), kind.title()))
            }
            Row::Item(item) => (section, item.label.clone()),
        })
        .collect()
}

#[cfg(test)]
pub(super) mod tests;
