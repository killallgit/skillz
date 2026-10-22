//! Which library items point at a local folder that is gone.

use super::rows::Row;
use crate::stale::{Probe, missing_source};

/// One flag per row, in row order; packages are never stale.
pub(super) fn stale_rows(rows: &[Row], probe: &dyn Probe) -> Vec<bool> {
    rows.iter()
        .map(|row| match row {
            Row::Item(item) => missing_source(&item.entry, probe).is_some(),
            Row::Package { .. } => false,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::rows::{build, tests::skill};
    use super::*;
    use crate::library::Catalog;
    use crate::stale::tests::Existing;

    #[test]
    fn an_item_with_a_vanished_source_is_flagged_and_packages_never_are() {
        let items = vec![skill("/gone", "a"), skill("/here", "b")];
        let rows = build(&Catalog::from_items(items));

        let flags = stale_rows(&rows, &Existing::of(&["/here"]));

        assert_eq!(flags, [false, true, false]);
    }
}
