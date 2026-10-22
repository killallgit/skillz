use super::catalog::Catalog;
use super::import::collect_installed;
use super::item::LibraryItem;
use super::repoint::{Moved, repoint};
use crate::model::Augmentation;
use crate::snapshot::{Reason, Skipped};
use crate::stale::Probe;

/// What a sync found that the library did not have, and what it had to leave out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Synced {
    pub added: Vec<LibraryItem>,
    pub skipped: Vec<Skipped>,
    /// Stale items pointed at the new place of their folder.
    pub repointed: Vec<Moved>,
    /// Labels of stale items that several live sources could replace, left as they are.
    pub ambiguous: Vec<String>,
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

/// Add to `catalog` the installed `items` it does not know yet, marked as detected. Items
/// already there keep their label and provenance; the only one ever dropped is a stale
/// duplicate of a live item (see `repoint`), and stale hand-made items are pointed at the live
/// item that replaces them instead of being added beside it.
pub fn sync_installed(
    catalog: &mut Catalog,
    items: &[Augmentation],
    (recorded_at, probe): (u64, &dyn Probe),
) -> Synced {
    let (found, skipped) = collect_installed(items, recorded_at);
    let found: Vec<LibraryItem> = found.into_iter().map(mark_detected).collect();
    let moves = repoint(catalog, &found, probe);
    let added = found
        .into_iter()
        .filter(|item| catalog.add_if_new(item.clone()))
        .collect();
    Synced {
        added,
        skipped,
        repointed: moves.moved,
        ambiguous: moves.ambiguous,
    }
}

fn mark_detected(item: LibraryItem) -> LibraryItem {
    let provenance = item.provenance.map(|p| p.detected());
    LibraryItem { provenance, ..item }
}

#[cfg(test)]
mod tests {
    use super::super::import::tests::installed_skill;
    use super::*;
    use crate::model::{Installer, Origin};
    use crate::profile::Entry;
    use crate::stale::Disk;

    fn at(recorded_at: u64) -> (u64, &'static dyn Probe) {
        (recorded_at, &Disk)
    }

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

        let synced = sync_installed(&mut catalog, &[installed_skill("tdd", Some("o/r"))], at(7));

        let provenance = catalog.items()[0].provenance.clone().unwrap();
        assert_eq!((synced.added.len(), provenance.detected), (1, true));
    }

    #[test]
    fn syncing_again_adds_nothing_and_changes_nothing() {
        let mut catalog = Catalog::default();
        let items = [installed_skill("tdd", Some("o/r"))];
        sync_installed(&mut catalog, &items, at(7));
        let first = catalog.clone();

        let again = sync_installed(&mut catalog, &items, at(99));

        assert_eq!((again.added.len(), catalog), (0, first));
    }

    #[test]
    fn items_no_longer_installed_stay_in_the_library() {
        let mut catalog = Catalog::default();
        sync_installed(&mut catalog, &[installed_skill("tdd", Some("o/r"))], at(7));

        sync_installed(&mut catalog, &[], at(8));

        assert_eq!(catalog.items().len(), 1);
    }

    #[test]
    fn a_manual_item_keeps_its_label_and_stays_without_provenance() {
        let mut catalog = Catalog::from_items(vec![manual("my favourite")]);

        let synced = sync_installed(&mut catalog, &[installed_skill("tdd", Some("o/r"))], at(7));

        assert!(synced.added.is_empty());
        assert_eq!(catalog.items(), [manual("my favourite")]);
    }

    #[test]
    fn a_hand_made_skill_is_added_as_detected_manual_with_its_dir_as_the_link() {
        let mut catalog = Catalog::default();
        let mine = Augmentation {
            installer: Installer::Local,
            origin: Origin {
                link: Some("/p/.agents/skills".into()),
                version: None,
            },
            ..installed_skill("mine", None)
        };

        let synced = sync_installed(&mut catalog, &[mine], at(7));

        let item = &catalog.items()[0];
        let provenance = item.provenance.clone().unwrap();
        assert!(synced.skipped.is_empty());
        assert_eq!(
            (provenance.engine, provenance.detected, provenance.link),
            (Installer::Local, true, Some("/p/.agents/skills".into()))
        );
        assert!(
            matches!(&item.entry, Entry::Skill { source, .. } if source == "/p/.agents/skills")
        );
    }

    #[test]
    fn what_cannot_be_kept_is_reported_and_not_stored() {
        let mut catalog = Catalog::default();

        let synced = sync_installed(&mut catalog, &[installed_skill("mine", None)], at(7));

        assert_eq!((catalog.items().len(), synced.skipped.len()), (0, 1));
    }

    #[test]
    fn a_moved_hand_made_skill_is_pointed_at_its_new_dir_and_not_added_twice() {
        use crate::stale::tests::{Existing, skill};
        let stale = LibraryItem::from_entry(skill("/old/skills", "mine"));
        let mut catalog = Catalog::from_items(vec![stale]);
        let moved_skill = Augmentation {
            installer: Installer::Local,
            origin: Origin {
                link: Some("/new/skills".into()),
                version: None,
            },
            ..installed_skill("mine", None)
        };

        let synced = sync_installed(
            &mut catalog,
            &[moved_skill],
            (7, &Existing::of(&["/new/skills"])),
        );

        assert_eq!((synced.repointed.len(), synced.added.len()), (1, 0));
        assert_eq!(catalog.items().len(), 1);
        assert_eq!(catalog.items()[0].entry, skill("/new/skills", "mine"));
    }
}
