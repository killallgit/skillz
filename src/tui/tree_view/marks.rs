//! Marking rows, and turning the marks into a `Picked`.

use std::collections::BTreeSet;

use super::model::TreeState;
use crate::prompt::Picked;

/// Whether a row is chosen: all of it, only part of it (a group with some items marked), or not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mark {
    No,
    Part,
    All,
}

impl TreeState {
    /// The markable row `id` belongs to: itself, its item, or its group.
    fn target(&self, mut id: usize) -> usize {
        while self.rows[id].depth > 0 && self.rows[id].item.is_none() {
            id = self.rows[id].parent.expect("non-top rows have a parent");
        }
        id
    }

    fn group_of(&self, mut id: usize) -> usize {
        while let Some(p) = self.rows[id].parent {
            id = p;
        }
        id
    }

    fn items_in(&self, group: usize) -> Vec<usize> {
        (0..self.rows.len())
            .filter(|&i| self.rows[i].item.is_some() && self.group_of(i) == group)
            .collect()
    }

    fn set_mark(&mut self, id: usize, on: bool) {
        if on {
            self.marks.insert(id);
        } else {
            self.marks.remove(&id);
        }
    }

    /// x on a group marks or unmarks all its items; on an item, just that item.
    pub fn toggle_mark(&mut self) {
        let Some(id) = self.current() else { return };
        let id = self.target(id);
        let on = !self.marks.contains(&id);
        if self.rows[id].depth == 0 {
            for i in self.items_in(id).into_iter().chain([id]) {
                self.set_mark(i, on);
            }
        } else {
            self.set_mark(id, on);
            if !on {
                self.marks.remove(&self.group_of(id));
            }
        }
    }

    /// Marked groups (as top-level ordinals) and items outside them; with no marks, whatever
    /// the cursor is on.
    pub fn picked(&self) -> Picked {
        let marks: BTreeSet<usize> = match self.current() {
            Some(id) if self.marks.is_empty() => [self.target(id)].into(),
            _ => self.marks.clone(),
        };
        let tops: Vec<usize> = (0..self.rows.len())
            .filter(|&i| self.rows[i].depth == 0)
            .collect();
        Picked {
            groups: (0..tops.len())
                .filter(|&o| marks.contains(&tops[o]))
                .collect(),
            items: marks
                .iter()
                .filter(|&&i| !marks.contains(&self.group_of(i)))
                .filter_map(|&i| self.rows[i].item)
                .collect(),
        }
    }

    pub(super) fn mark(&self, id: usize) -> Mark {
        if self.marks.contains(&id) {
            Mark::All
        } else if self.rows[id].depth == 0
            && self.items_in(id).iter().any(|i| self.marks.contains(i))
        {
            Mark::Part
        } else {
            Mark::No
        }
    }
}
