//! The rows of the project outline: a section per kind, holding what the project has installed
//! and then the library items it does not have yet.

use crate::apply::compare;
use crate::library::{Catalog, ItemKind, LibraryItem};
use crate::model::Augmentation;
use crate::profile::{Entry, Profile};
use crate::tui::widgets::outline::Shape;

/// The sections in the order they are shown.
const SECTIONS: [ItemKind; 5] = [
    ItemKind::Skill,
    ItemKind::Plugin,
    ItemKind::Mcp,
    ItemKind::Hook,
    ItemKind::Subagent,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum State {
    /// Installed; the id of the item to remove.
    Installed(String),
    /// In the library only; the entry that installs it.
    Available(Entry),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Item {
    pub label: String,
    /// Where an installed item came from, shown beside its name.
    pub detail: String,
    pub state: State,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Row {
    Section {
        kind: ItemKind,
        installed: usize,
        total: usize,
    },
    Item(Item),
}

/// Every section that has something, each followed by its items, in preorder.
pub(super) fn build(installed: &[Augmentation], library: &Catalog) -> Vec<Row> {
    let available: Vec<&LibraryItem> = library
        .items()
        .iter()
        .filter(|item| !is_installed(item, installed))
        .collect();
    SECTIONS
        .into_iter()
        .flat_map(|kind| section(kind, installed, &available))
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

fn section(kind: ItemKind, installed: &[Augmentation], available: &[&LibraryItem]) -> Vec<Row> {
    let mut have: Vec<&Augmentation> = installed
        .iter()
        .filter(|a| ItemKind::of(a.kind) == Some(kind))
        .collect();
    have.sort_by_key(|a| a.name.to_lowercase());
    let mut lacking: Vec<&LibraryItem> = available
        .iter()
        .copied()
        .filter(|item| item.kind() == kind)
        .collect();
    lacking.sort_by_key(|item| item.label.to_lowercase());
    if have.is_empty() && lacking.is_empty() {
        return vec![];
    }
    let header = Row::Section {
        kind,
        installed: have.len(),
        total: have.len() + lacking.len(),
    };
    let items = have
        .into_iter()
        .map(installed_item)
        .chain(lacking.into_iter().map(available_item));
    std::iter::once(header).chain(items).collect()
}

fn installed_item(item: &Augmentation) -> Row {
    Row::Item(Item {
        label: item.name.clone(),
        detail: item.source.clone().unwrap_or_default(),
        state: State::Installed(item.id.clone()),
    })
}

fn available_item(item: &LibraryItem) -> Row {
    Row::Item(Item {
        label: item.label.clone(),
        detail: String::new(),
        state: State::Available(item.entry.clone()),
    })
}

/// Whether everything the library item would install is already in place.
fn is_installed(item: &LibraryItem, installed: &[Augmentation]) -> bool {
    let mut profile = Profile::new("library");
    profile.entries = vec![item.entry.clone()];
    compare(&profile.resolved(), installed).missing.is_empty()
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::model::{Agent, Installer, Kind, Scope};

    pub fn installed(name: &str, kind: Kind, source: Option<&str>) -> Augmentation {
        Augmentation {
            hooks: vec![],
            id: format!("{kind}:{name}"),
            name: name.into(),
            kind,
            agents: vec![Agent::Claude, Agent::Codex],
            scope: Scope::Project,
            group: source.unwrap_or("local").into(),
            source: source.map(Into::into),
            installer: Installer::Npx,
            origin: Default::default(),
            children: vec![],
            removal: vec![],
        }
    }

    pub fn library_skill(source: &str, name: &str) -> LibraryItem {
        LibraryItem::from_entry(Entry::Skill {
            source: source.into(),
            names: vec![name.into()],
            agents: vec![],
        })
    }

    fn sections_of(rows: &[Row]) -> Vec<(ItemKind, usize, usize)> {
        rows.iter()
            .filter_map(|row| match row {
                Row::Section {
                    kind,
                    installed,
                    total,
                } => Some((*kind, *installed, *total)),
                Row::Item(_) => None,
            })
            .collect()
    }

    #[test]
    fn a_section_counts_what_is_installed_of_everything_it_offers() {
        let installed = [installed("tdd", Kind::Skill, Some("o/r"))];
        let library = Catalog::from_items(vec![library_skill("o/r", "grill")]);

        let rows = build(&installed, &library);

        assert_eq!(sections_of(&rows), [(ItemKind::Skill, 1, 2)]);
    }

    #[test]
    fn a_library_item_that_is_installed_is_listed_once() {
        let installed = [installed("tdd", Kind::Skill, Some("o/r"))];
        let library = Catalog::from_items(vec![library_skill("o/r", "tdd")]);

        let rows = build(&installed, &library);

        assert_eq!(rows.len(), 2);
        assert!(matches!(&rows[1], Row::Item(i) if matches!(i.state, State::Installed(_))));
    }

    #[test]
    fn installed_items_come_before_available_ones() {
        let installed = [installed("zed", Kind::Skill, None)];
        let library = Catalog::from_items(vec![library_skill("o/r", "aaa")]);

        let rows = build(&installed, &library);

        let states: Vec<bool> = rows
            .iter()
            .filter_map(|r| match r {
                Row::Item(i) => Some(matches!(i.state, State::Installed(_))),
                Row::Section { .. } => None,
            })
            .collect();
        assert_eq!(states, [true, false]);
    }

    #[test]
    fn sections_follow_the_display_order_and_empty_ones_are_left_out() {
        let installed = [
            installed("h", Kind::Hook, None),
            installed("s", Kind::Skill, None),
            installed("m", Kind::Marketplace, None),
        ];

        let rows = build(&installed, &Catalog::default());

        let kinds: Vec<ItemKind> = sections_of(&rows).into_iter().map(|s| s.0).collect();
        assert_eq!(kinds, [ItemKind::Skill, ItemKind::Hook]);
    }

    #[test]
    fn items_hang_under_the_section_before_them() {
        let installed = [
            installed("a", Kind::Skill, None),
            installed("b", Kind::Hook, None),
        ];

        let shape = shape(&build(&installed, &Catalog::default()));

        let parents: Vec<Option<usize>> = shape.iter().map(|(p, _)| *p).collect();
        assert_eq!(parents, [None, Some(0), None, Some(2)]);
    }
}
