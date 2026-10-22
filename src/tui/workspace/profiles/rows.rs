//! The rows of an entry outline: a section per kind, holding the entries of that kind. Shown for
//! a profile's entries and for the library items a profile can be built from.

use crate::library::{Catalog, ItemKind};
use crate::profile::{Entry, Profile};
use crate::tui::widgets::outline::Shape;

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

/// One line of the profile list: a profile, or one of its entries beneath it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ListRow {
    Profile(usize),
    Entry { profile: usize, label: String },
}

impl ListRow {
    /// The index of the profile the row is, or belongs to.
    pub fn profile(&self) -> usize {
        match self {
            ListRow::Profile(at) | ListRow::Entry { profile: at, .. } => *at,
        }
    }
}

/// Each profile followed by its entries.
pub(super) fn of_profiles<'a>(profiles: impl Iterator<Item = &'a Profile>) -> Vec<ListRow> {
    profiles
        .enumerate()
        .flat_map(|(at, profile)| {
            let entries = profile.entries.iter().map(move |entry| ListRow::Entry {
                profile: at,
                label: entry.to_string(),
            });
            std::iter::once(ListRow::Profile(at)).chain(entries)
        })
        .collect()
}

/// The list's shape: entries hang under the profile before them.
pub(super) fn list_shape(rows: &[ListRow], names: &[String]) -> Vec<Shape> {
    let mut profile = None;
    rows.iter()
        .enumerate()
        .map(|(id, row)| match row {
            ListRow::Profile(at) => {
                profile = Some(id);
                (None, names[*at].clone())
            }
            ListRow::Entry { label, .. } => (profile, label.clone()),
        })
        .collect()
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

    #[test]
    fn each_profile_is_followed_by_its_entries() {
        let mut web = Profile::new("web");
        web.entries = vec![skill("a"), skill("b")];
        let api = Profile::new("api");

        let rows = of_profiles([&web, &api].into_iter());

        assert_eq!(
            rows.iter().map(ListRow::profile).collect::<Vec<_>>(),
            [0, 0, 0, 1]
        );
        let names = ["web".to_string(), "api".to_string()];
        let parents: Vec<_> = list_shape(&rows, &names).into_iter().map(|s| s.0).collect();
        assert_eq!(parents, [None, Some(0), Some(0), None]);
    }

    #[test]
    fn library_items_keep_their_label_and_hang_under_their_section() {
        let library = Catalog::from_items(vec![LibraryItem::new("mine", skill("a"))]);

        let rows = of_library(&library);

        assert!(matches!(&rows[1], Row::Item(item) if item.label == "mine"));
        assert_eq!(shape(&rows)[1].0, Some(0));
    }

}
