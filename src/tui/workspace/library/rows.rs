//! The rows of the library outline: a group per source package, holding its items.

use crate::app::library::{Catalog, Held, ItemKind, LibraryItem, Presence};
use crate::app::package::{Package, group_by};
use crate::app::stale::{Probe, missing_source};
use crate::tui::widgets::outline::Shape;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Row {
    Agents,
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
    let (agents, items): (Vec<_>, Vec<_>) = catalog
        .items()
        .iter()
        .partition(|item| item.kind() == ItemKind::Subagent);
    let mut rows = package_rows(items, held, probe, |item| Package::of_entry(&item.entry));
    if !agents.is_empty() {
        rows.push(Row::Agents);
        rows.extend(package_rows(agents, held, probe, |item| {
            Package::Source(item.source())
        }));
    }
    rows
}

fn package_rows(
    items: Vec<&LibraryItem>,
    held: &Held,
    probe: &dyn Probe,
    package: impl Fn(&LibraryItem) -> Package,
) -> Vec<Row> {
    group_by(items, |item| package(item))
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

/// Agents have their own section; each source package holds its items.
pub(super) fn shape(rows: &[Row]) -> Vec<Shape> {
    let (mut section, mut group) = (None, None);
    rows.iter()
        .enumerate()
        .map(|(id, row)| match row {
            Row::Agents => {
                section = Some(id);
                group = None;
                (None, "Agents".into())
            }
            Row::Package { package } => {
                group = Some(id);
                (section, package.title())
            }
            Row::Item(item) => (group, item.library.short_label()),
        })
        .collect()
}

#[cfg(test)]
pub(super) mod tests;
