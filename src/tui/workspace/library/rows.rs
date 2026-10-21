//! The rows of the library outline: a section per kind, holding its items.

use crate::library::{Catalog, ItemKind, LibraryItem};
use crate::tui::widgets::outline::Shape;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Row {
    Section { kind: ItemKind, count: usize },
    Item(Box<LibraryItem>),
}

/// Every kind that has items, each followed by them, in preorder.
pub(super) fn build(catalog: &Catalog) -> Vec<Row> {
    catalog
        .grouped()
        .into_iter()
        .flat_map(|(kind, items)| {
            let section = Row::Section {
                kind,
                count: items.len(),
            };
            let items = items
                .into_iter()
                .map(|item| Row::Item(Box::new(item.clone())));
            std::iter::once(section).chain(items)
        })
        .collect()
}

/// The outline's shape: items hang under the section before them.
pub(super) fn shape(rows: &[Row]) -> Vec<Shape> {
    let mut section = None;
    rows.iter()
        .enumerate()
        .map(|(id, row)| match row {
            Row::Section { kind, .. } => {
                section = Some(id);
                (None, kind.title().to_string())
            }
            Row::Item(item) => (section, item.label.clone()),
        })
        .collect()
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::profile::Entry;

    pub fn skill(source: &str, name: &str) -> LibraryItem {
        LibraryItem::from_entry(Entry::Skill {
            source: source.into(),
            names: vec![name.into()],
            agents: vec![],
        })
    }

    pub fn hook(command: &str) -> LibraryItem {
        LibraryItem::from_entry(Entry::Hook {
            event: "Stop".into(),
            matcher: None,
            command: command.into(),
            agents: vec![],
        })
    }

    #[test]
    fn items_hang_under_a_section_of_their_kind() {
        let catalog = Catalog::from_items(vec![hook("x"), skill("o/r", "b"), skill("o/r", "a")]);

        let rows = build(&catalog);

        let parents: Vec<Option<usize>> = shape(&rows).into_iter().map(|(p, _)| p).collect();
        assert_eq!(parents, [None, Some(0), Some(0), None, Some(3)]);
        assert!(matches!(rows[0], Row::Section { count: 2, .. }));
    }

    #[test]
    fn an_empty_library_has_no_rows() {
        assert!(build(&Catalog::default()).is_empty());
    }
}
