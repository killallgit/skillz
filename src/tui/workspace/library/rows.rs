//! The rows of the library outline: a group per source package, holding its items.

use crate::library::{Catalog, Held, LibraryItem, Presence};
use crate::package::{Package, group_by};
use crate::stale::{Probe, missing_source};
use crate::tui::widgets::outline::Shape;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Row {
    Package { package: Package },
    Item(Box<Item>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Item {
    pub library: LibraryItem,
    pub presence: Presence,
    pub stale: bool,
}

/// Every package that has items, each followed by them, in preorder.
pub(super) fn build(catalog: &Catalog, held: &Held, probe: &dyn Probe) -> Vec<Row> {
    let items: Vec<&LibraryItem> = catalog.items().iter().collect();
    group_by(items, |item| Package::of_entry(&item.entry))
        .into_iter()
        .flat_map(|(package, mut items)| {
            items.sort_by_key(|item| (item.kind(), item.short_label().to_lowercase()));
            let group = Row::Package { package };
            let items = items.into_iter().map(|item| {
                Row::Item(Box::new(Item {
                    presence: held.presence(&item.entry),
                    stale: missing_source(&item.entry, probe).is_some(),
                    library: item.clone(),
                }))
            });
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
            Row::Package { package } => {
                group = Some(id);
                (None, package.title())
            }
            Row::Item(item) => (group, item.library.short_label()),
        })
        .collect()
}

#[cfg(test)]
pub(super) mod tests;
