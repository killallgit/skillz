use serde::{Deserialize, Serialize};

use super::item::LibraryItem;
use crate::profile::{Entry, same_thing};

/// The items of a library, in the order they were first added.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "Vec<LibraryItem>", into = "Vec<LibraryItem>")]
pub struct Catalog {
    items: Vec<LibraryItem>,
}

impl Catalog {
    pub fn from_items(items: Vec<LibraryItem>) -> Self {
        items.into_iter().fold(Catalog::default(), |mut c, item| {
            c.add(item);
            c
        })
    }

    pub fn items(&self) -> &[LibraryItem] {
        &self.items
    }

    /// Replace the item that is the same thing in place, else append. An item without a
    /// provenance keeps the one it replaces.
    pub fn add(&mut self, item: LibraryItem) {
        match self.items.iter_mut().find(|i| i.is_same_thing_as(&item)) {
            Some(existing) => {
                let provenance = item.provenance.or(existing.provenance.take());
                *existing = LibraryItem { provenance, ..item };
            }
            None => self.items.push(item),
        }
    }

    /// Append `item` unless the library already has the same thing, leaving that one as it is;
    /// whether it was appended.
    pub fn add_if_new(&mut self, item: LibraryItem) -> bool {
        let known = self.items.iter().any(|i| i.is_same_thing_as(&item));
        if !known {
            self.items.push(item);
        }
        !known
    }

    /// Add each thing `entry` names as an item of its own.
    pub fn add_entry(&mut self, entry: Entry) {
        LibraryItem::each_of(entry)
            .into_iter()
            .for_each(|item| self.add(item));
    }

    /// The entry to install again: a skill is fetched from the link recorded when it was last
    /// installed, when there is a usable one, else from the entry's own source.
    pub fn installable(&self, entry: &Entry) -> Entry {
        let link = self
            .items
            .iter()
            .find(|i| same_thing(&i.entry, entry))
            .and_then(|i| i.provenance.as_ref()?.link.clone());
        match (entry, link) {
            (Entry::Skill { names, agents, .. }, Some(source)) => {
                let linked = Entry::Skill {
                    source,
                    names: names.clone(),
                    agents: agents.clone(),
                };
                if linked.validate().is_ok() {
                    linked
                } else {
                    entry.clone()
                }
            }
            _ => entry.clone(),
        }
    }

    /// Put `new` where `old` is, keeping its position.
    pub(super) fn replace(&mut self, old: &LibraryItem, new: LibraryItem) {
        if let Some(slot) = self.items.iter_mut().find(|i| i.is_same_thing_as(old)) {
            *slot = new;
        }
    }

    /// Remove the item that is the same thing as `item`; whether there was one.
    pub fn remove(&mut self, item: &LibraryItem) -> bool {
        let before = self.items.len();
        self.items.retain(|i| !i.is_same_thing_as(item));
        self.items.len() != before
    }
}

impl From<Vec<LibraryItem>> for Catalog {
    fn from(items: Vec<LibraryItem>) -> Self {
        Catalog::from_items(items)
    }
}

impl From<Catalog> for Vec<LibraryItem> {
    fn from(catalog: Catalog) -> Self {
        catalog.items
    }
}

#[cfg(test)]
mod tests {
    use super::super::item::tests::{hook, skill};
    use super::*;

    #[test]
    fn adding_the_same_thing_again_updates_it_in_place() {
        let mut catalog = Catalog::from_items(vec![skill("o/r", "a"), hook("x")]);
        let renamed = LibraryItem::new("renamed", skill("o/r", "a").entry);

        catalog.add(renamed.clone());

        // Assert one item per thing, the update keeping the original position.
        assert_eq!(catalog.items(), [renamed, hook("x")]);
    }

    #[test]
    fn removing_drops_the_item_and_reports_whether_it_was_there() {
        let mut catalog = Catalog::from_items(vec![skill("o/r", "a")]);

        let first = catalog.remove(&skill("o/r", "a"));
        let second = catalog.remove(&skill("o/r", "a"));

        // Assert only the first removal found something.
        assert_eq!((first, second, catalog.items().len()), (true, false, 0));
    }
}

#[cfg(test)]
mod split_tests {
    use super::*;
    use crate::profile::Entry;

    fn skills(names: &[&str]) -> Entry {
        Entry::Skill {
            source: "o/r".into(),
            names: names.iter().map(|n| n.to_string()).collect(),
            agents: vec![],
        }
    }

    #[test]
    fn an_entry_naming_several_things_becomes_one_item_each() {
        let mut catalog = Catalog::default();

        catalog.add_entry(skills(&["grill", "tdd"]));

        assert_eq!(catalog.items().len(), 2);
    }

    #[test]
    fn adding_part_of_what_is_there_keeps_the_rest() {
        let mut catalog = Catalog::default();
        catalog.add_entry(skills(&["grill", "tdd"]));

        catalog.add_entry(skills(&["tdd"]));

        // Assert nothing was lost: grill and tdd are still one item each.
        assert_eq!(catalog.items().len(), 2);
    }
}
