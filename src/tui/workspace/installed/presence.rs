//! Whether an item is in place, by the same comparison a profile uses against the inventory.

use crate::apply::holds;
use crate::library::LibraryItem;
use crate::model::Augmentation;
use crate::profile::Entry;
use crate::snapshot::snapshot;

/// Whether the library item is already in place among `have`.
pub(super) fn has_library_item(have: &[Augmentation], item: &LibraryItem) -> bool {
    holds(have, &item.entry)
}

/// Whether what `item` puts in place is also in place among `have`; an item no profile could
/// reproduce is never matched.
pub(super) fn has_counterpart(have: &[Augmentation], item: &Augmentation) -> bool {
    entry_of(item).is_some_and(|entry| holds(have, &entry))
}

/// The entry that would install `item` again, when a profile can express it.
fn entry_of(item: &Augmentation) -> Option<Entry> {
    snapshot("installed", std::slice::from_ref(item))
        .profile
        .entries
        .into_iter()
        .next()
}

#[cfg(test)]
mod tests {
    use super::super::rows::tests::{installed, library_skill};
    use super::*;
    use crate::model::Kind;

    #[test]
    fn a_library_item_is_found_among_what_is_installed() {
        let have = [installed("tdd", Kind::Skill, Some("o/r"))];

        let found = has_library_item(&have, &library_skill("o/r", "tdd"));
        let other = has_library_item(&have, &library_skill("o/r", "grill"));

        assert_eq!((found, other), (true, false));
    }

    #[test]
    fn an_installed_item_is_found_by_what_it_would_reinstall() {
        let have = [installed("tdd", Kind::Skill, Some("o/r"))];
        let same = installed("tdd", Kind::Skill, Some("o/r"));
        let other = installed("grill", Kind::Skill, Some("o/r"));

        assert!(has_counterpart(&have, &same));
        assert!(!has_counterpart(&have, &other));
    }

    #[test]
    fn an_item_nothing_could_reinstall_has_no_entry_and_no_counterpart() {
        let local = installed("mine", Kind::Skill, None);

        assert!(entry_of(&local).is_none());
        assert!(!has_counterpart(std::slice::from_ref(&local), &local));
    }
}
