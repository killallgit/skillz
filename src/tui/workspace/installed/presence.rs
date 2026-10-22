//! Whether an item is in place, by the same comparison a profile uses against the inventory.

use crate::apply::holds;
use crate::model::Augmentation;
use crate::snapshot::entries;

/// Whether what `item` puts in place is also in place among `have`; an item no profile could
/// reproduce is never matched.
pub(super) fn has_counterpart(have: &[Augmentation], item: &Augmentation) -> bool {
    let mut found = false;
    for part in item.parts() {
        let Ok(entries) = entries(part) else {
            return false;
        };
        for entry in entries {
            found = true;
            if !holds(have, &entry) {
                return false;
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::super::rows::tests::installed;
    use super::*;
    use crate::model::Kind;

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

        assert!(!has_counterpart(std::slice::from_ref(&local), &local));
    }
}
