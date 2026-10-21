use super::item::{ItemKind, LibraryItem};

/// The items of a library, in the order they were first added.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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

    /// Replace the item that is the same thing in place, else append.
    pub fn add(&mut self, item: LibraryItem) {
        match self.items.iter_mut().find(|i| i.is_same_thing_as(&item)) {
            Some(existing) => *existing = item,
            None => self.items.push(item),
        }
    }

    /// Remove the item that is the same thing as `item`; whether there was one.
    pub fn remove(&mut self, item: &LibraryItem) -> bool {
        let before = self.items.len();
        self.items.retain(|i| !i.is_same_thing_as(item));
        self.items.len() != before
    }

    /// Items grouped by kind in kind order, each group sorted by label.
    pub fn grouped(&self) -> Vec<(ItemKind, Vec<&LibraryItem>)> {
        let mut sorted: Vec<&LibraryItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| (a.kind(), &a.label).cmp(&(b.kind(), &b.label)));
        let mut groups: Vec<(ItemKind, Vec<&LibraryItem>)> = vec![];
        for item in sorted {
            match groups.last_mut() {
                Some((kind, members)) if *kind == item.kind() => members.push(item),
                _ => groups.push((item.kind(), vec![item])),
            }
        }
        groups
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

    #[test]
    fn grouping_orders_kinds_then_labels_whatever_the_insertion_order() {
        let (a, b, h) = (skill("o/r", "a"), skill("o/r", "b"), hook("x"));
        let catalog = Catalog::from_items(vec![h.clone(), b.clone(), a.clone()]);

        let groups = catalog.grouped();

        // Assert skills come before hooks and each group is label sorted.
        assert_eq!(
            groups,
            [(ItemKind::Skill, vec![&a, &b]), (ItemKind::Hook, vec![&h])]
        );
    }
}
