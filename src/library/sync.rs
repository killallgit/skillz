use super::catalog::Catalog;
use super::import::collect_installed;
use super::item::LibraryItem;
use crate::model::Augmentation;
use crate::snapshot::{Reason, Skipped};

/// What a sync found that the library did not have, and what it had to leave out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Synced {
    pub added: Vec<LibraryItem>,
    pub skipped: Vec<Skipped>,
}

impl Synced {
    /// How many installed items were left out because they need secrets.
    pub fn secrets_skipped(&self) -> usize {
        self.skipped
            .iter()
            .filter(|s| matches!(s.reason, Reason::Secrets(_)))
            .count()
    }
}

/// Add to `catalog` the installed `items` it does not know yet, marked as detected. Only
/// appends: items already there keep their label and provenance, and none is ever dropped.
pub fn sync_installed(catalog: &mut Catalog, items: &[Augmentation], recorded_at: u64) -> Synced {
    let (found, skipped) = collect_installed(items, recorded_at);
    let added = found
        .into_iter()
        .map(mark_detected)
        .filter(|item| catalog.add_if_new(item.clone()))
        .collect();
    Synced { added, skipped }
}

fn mark_detected(item: LibraryItem) -> LibraryItem {
    let provenance = item.provenance.map(|p| p.detected());
    LibraryItem { provenance, ..item }
}

#[cfg(test)]
mod tests {
    use super::super::import::tests::installed_skill;
    use super::*;
    use crate::profile::Entry;

    fn manual(label: &str) -> LibraryItem {
        LibraryItem::new(
            label,
            Entry::Skill {
                source: "o/r".into(),
                names: vec!["tdd".into()],
                agents: vec![],
            },
        )
    }

    #[test]
    fn a_found_item_is_added_marked_as_detected() {
        let mut catalog = Catalog::default();

        let synced = sync_installed(&mut catalog, &[installed_skill("tdd", Some("o/r"))], 7);

        // Assert one new item whose provenance says it was detected.
        let provenance = catalog.items()[0].provenance.clone().unwrap();
        assert_eq!((synced.added.len(), provenance.detected), (1, true));
    }

    #[test]
    fn syncing_again_adds_nothing_and_changes_nothing() {
        let mut catalog = Catalog::default();
        let items = [installed_skill("tdd", Some("o/r"))];
        sync_installed(&mut catalog, &items, 7);
        let first = catalog.clone();

        let again = sync_installed(&mut catalog, &items, 99);

        assert_eq!((again.added.len(), catalog), (0, first));
    }

    #[test]
    fn items_no_longer_installed_stay_in_the_library() {
        let mut catalog = Catalog::default();
        sync_installed(&mut catalog, &[installed_skill("tdd", Some("o/r"))], 7);

        sync_installed(&mut catalog, &[], 8);

        assert_eq!(catalog.items().len(), 1);
    }

    #[test]
    fn a_manual_item_keeps_its_label_and_stays_without_provenance() {
        let mut catalog = Catalog::from_items(vec![manual("my favourite")]);

        let synced = sync_installed(&mut catalog, &[installed_skill("tdd", Some("o/r"))], 7);

        // Assert nothing was added and the manual entry is untouched.
        assert!(synced.added.is_empty());
        assert_eq!(catalog.items(), [manual("my favourite")]);
    }

    #[test]
    fn what_cannot_be_kept_is_reported_and_not_stored() {
        let mut catalog = Catalog::default();

        let synced = sync_installed(&mut catalog, &[installed_skill("mine", None)], 7);

        assert_eq!((catalog.items().len(), synced.skipped.len()), (0, 1));
    }
}
