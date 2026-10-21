//! The rows of an entry outline: a section per kind, holding the entries of that kind. Shown for
//! a profile's entries and for the library items a profile can be built from.

use crate::library::{Catalog, ItemKind};
use crate::profile::Entry;
use crate::tui::widgets::outline::{Outline, Shape};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Item {
    pub label: String,
    pub entry: Entry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Row {
    Section { kind: ItemKind, count: usize },
    Item(Item),
}

pub(super) fn section_title(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Skill => "Skills",
        ItemKind::Plugin => "Plugins",
        ItemKind::Mcp => "MCP servers",
        ItemKind::Hook => "Hooks",
        ItemKind::Subagent => "Subagents",
    }
}

pub(super) fn of_entries(entries: &[Entry]) -> Vec<Row> {
    let items = entries.iter().map(|entry| Item {
        label: entry.to_string(),
        entry: entry.clone(),
    });
    grouped(items.collect())
}

pub(super) fn of_library(library: &Catalog) -> Vec<Row> {
    let items = library.items().iter().map(|item| Item {
        label: item.label.clone(),
        entry: item.entry.clone(),
    });
    grouped(items.collect())
}

/// The outline's shape: items hang under the section before them.
pub(super) fn shape(rows: &[Row]) -> Vec<Shape> {
    let mut section = None;
    rows.iter()
        .enumerate()
        .map(|(id, row)| match row {
            Row::Section { kind, .. } => {
                section = Some(id);
                (None, section_title(*kind).to_string())
            }
            Row::Item(item) => (section, item.label.clone()),
        })
        .collect()
}

/// An outline over `rows` with every section open.
pub(super) fn open_outline(rows: &[Row]) -> Outline {
    let mut outline = Outline::new(vec![]);
    outline.reshape(shape(rows));
    outline
}

/// Each kind's section followed by its items sorted by label.
fn grouped(mut items: Vec<Item>) -> Vec<Row> {
    let kind = |item: &Item| ItemKind::of_entry(&item.entry);
    items.sort_by_key(|item| (kind(item), item.label.to_lowercase()));
    items
        .chunk_by(|a, b| kind(a) == kind(b))
        .flat_map(|group| {
            let header = Row::Section {
                kind: kind(&group[0]),
                count: group.len(),
            };
            std::iter::once(header).chain(group.iter().cloned().map(Row::Item))
        })
        .collect()
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::library::LibraryItem;

    pub fn skill(name: &str) -> Entry {
        Entry::Skill {
            source: "o/r".into(),
            names: vec![name.into()],
            agents: vec![],
        }
    }

    fn hook() -> Entry {
        Entry::Hook {
            event: "Stop".into(),
            matcher: None,
            command: "x".into(),
            agents: vec![],
        }
    }

    #[test]
    fn entries_are_grouped_under_a_section_per_kind() {
        let rows = of_entries(&[hook(), skill("b"), skill("a")]);

        let kinds: Vec<Option<ItemKind>> = rows
            .iter()
            .map(|row| match row {
                Row::Section { kind, .. } => Some(*kind),
                Row::Item(_) => None,
            })
            .collect();
        assert_eq!(
            kinds,
            [
                Some(ItemKind::Skill),
                None,
                None,
                Some(ItemKind::Hook),
                None
            ]
        );
        assert_eq!(
            rows[0],
            Row::Section {
                kind: ItemKind::Skill,
                count: 2
            }
        );
    }

    #[test]
    fn items_within_a_kind_are_sorted_by_label() {
        let rows = of_entries(&[skill("b"), skill("a")]);

        let entries: Vec<&Entry> = rows
            .iter()
            .filter_map(|row| match row {
                Row::Item(item) => Some(&item.entry),
                Row::Section { .. } => None,
            })
            .collect();
        assert_eq!(entries, [&skill("a"), &skill("b")]);
    }

    #[test]
    fn library_items_keep_their_label_and_hang_under_their_section() {
        let library = Catalog::from_items(vec![LibraryItem::new("mine", skill("a"))]);

        let rows = of_library(&library);

        assert!(matches!(&rows[1], Row::Item(item) if item.label == "mine"));
        assert_eq!(shape(&rows)[1].0, Some(0));
    }

    #[test]
    fn nothing_makes_no_rows() {
        assert!(of_entries(&[]).is_empty());
    }
}
