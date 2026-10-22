//! The rows of the library outline: a group per source package, holding its items.

use crate::library::{Catalog, ItemKind, LibraryItem};
use crate::package::{Package, group_by};
use crate::tui::widgets::outline::Shape;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Row {
    Package {
        package: Package,
        contents: Vec<(ItemKind, usize)>,
    },
    Item(Box<LibraryItem>),
}

/// Every package that has items, each followed by them, in preorder.
pub(super) fn build(catalog: &Catalog) -> Vec<Row> {
    let items: Vec<&LibraryItem> = catalog.items().iter().collect();
    group_by(items, |item| Package::of_entry(&item.entry))
        .into_iter()
        .flat_map(|(package, mut items)| {
            items.sort_by_key(|item| (item.kind(), item.short_label().to_lowercase()));
            let group = Row::Package {
                package,
                contents: ItemKind::tally(items.iter().map(|item| item.kind())),
            };
            let items = items
                .into_iter()
                .map(|item| Row::Item(Box::new(item.clone())));
            std::iter::once(group).chain(items)
        })
        .collect()
}

/// The outline's shape: items hang under the package before them.
pub(super) fn shape(rows: &[Row]) -> Vec<Shape> {
    let mut group = None;
    rows.iter()
        .enumerate()
        .map(|(id, row)| match row {
            Row::Package { package, .. } => {
                group = Some(id);
                (None, package.title())
            }
            Row::Item(item) => (group, item.short_label()),
        })
        .collect()
}

#[cfg(test)]
pub(super) mod tests;
