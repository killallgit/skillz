//! A hand-made skill or subagent whose folder moved: its library item still points at the old
//! path, and the same thing is now installed from a new one. Instead of leaving a stale item
//! beside a duplicate, the item is pointed at the new path.
//!
//! Rules, so a move is never guessed at:
//! - only an item with a single name and an absolute local path that no longer exists is stale;
//! - it is re-pointed only when exactly one live local source holds the same kind and name,
//!   otherwise it stays stale and is reported as ambiguous;
//! - when the library already holds that live item, the stale one is dropped as its duplicate;
//! - git and URL sources are not paths and are never touched; nothing else leaves the library.

use std::path::Path;

use super::catalog::Catalog;
use super::item::{ItemKind, LibraryItem};
use crate::stale::{Probe, local_source, missing_source, sole_name, with_source};

/// One item, or profile entry, pointed at the new place of a folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moved {
    pub kind: ItemKind,
    pub name: String,
    pub from: String,
    pub to: String,
    /// The profiles whose entries were pointed at the new path too.
    pub profiles: Vec<String>,
}

#[derive(Debug, Default)]
pub(super) struct Repointed {
    pub moved: Vec<Moved>,
    /// Labels of the stale items that more than one live source could replace.
    pub ambiguous: Vec<String>,
}

/// Re-point the stale items of `catalog` that `found`, the live local items, replace.
pub(super) fn repoint(
    catalog: &mut Catalog,
    found: &[LibraryItem],
    probe: &dyn Probe,
) -> Repointed {
    let stale: Vec<LibraryItem> = catalog
        .items()
        .iter()
        .filter(|item| sole_name(&item.entry).is_some())
        .filter(|item| missing_source(&item.entry, probe).is_some())
        .cloned()
        .collect();
    let mut result = Repointed::default();
    for old in stale {
        let candidates = live_candidates(found, &old, probe);
        match candidates.as_slice() {
            [] => {}
            [live] => result.moved.push(move_to(catalog, &old, live)),
            _ => result.ambiguous.push(old.label),
        }
    }
    result
}

fn live_candidates<'a>(
    found: &'a [LibraryItem],
    old: &LibraryItem,
    probe: &dyn Probe,
) -> Vec<&'a LibraryItem> {
    let mut seen = vec![];
    found
        .iter()
        .filter(|f| f.kind() == old.kind() && sole_name(&f.entry) == sole_name(&old.entry))
        .filter(|f| local_source(&f.entry).is_some_and(|s| probe.exists(Path::new(s))))
        .filter(|f| {
            let source = local_source(&f.entry);
            let fresh = !seen.contains(&source);
            seen.push(source);
            fresh
        })
        .collect()
}

fn move_to(catalog: &mut Catalog, old: &LibraryItem, live: &LibraryItem) -> Moved {
    let to = local_source(&live.entry).unwrap_or_default().to_string();
    let entry = with_source(old.entry.clone(), &to);
    let moved = Moved {
        kind: old.kind(),
        name: sole_name(&old.entry).unwrap_or_default().to_string(),
        from: local_source(&old.entry).unwrap_or_default().to_string(),
        to,
        profiles: vec![],
    };
    if catalog.items().iter().any(|i| i.entry == entry) {
        catalog.remove(old);
        return moved;
    }
    let label = if old.label == old.entry.to_string() {
        entry.to_string()
    } else {
        old.label.clone()
    };
    let provenance = live.provenance.clone();
    catalog.replace(
        old,
        LibraryItem {
            label,
            entry,
            provenance,
        },
    );
    moved
}
