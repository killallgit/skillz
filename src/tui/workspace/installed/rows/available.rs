//! The library items the scope does not have yet, each with the package it comes from.

use super::{Item, Sources, State};
use crate::apply::holds;
use crate::library::LibraryItem;
use crate::package::Package;

pub(super) fn items(sources: &Sources) -> Vec<(Package, Item)> {
    sources
        .library
        .items()
        .iter()
        .filter(|item| !holds(sources.here, &item.entry))
        .map(|item| (Package::of_entry(&item.entry), item_of(item, sources)))
        .collect()
}

fn item_of(item: &LibraryItem, sources: &Sources) -> Item {
    Item {
        label: item.short_label(),
        kind: item.kind(),
        agents: String::new(),
        detail: String::new(),
        state: State::Available(item.entry.clone()),
        global: holds(sources.also_global, &item.entry),
        owned: vec![],
    }
}
