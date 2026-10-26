//! The removals picked but not yet applied. An item is identified by its id, so a pick survives
//! a reload for as long as the item is still there.

use super::rows::{Item, Row};

#[derive(Debug, Clone, Default)]
pub(super) struct Pending {
    picked: Vec<String>,
}

impl Pending {
    pub fn is_empty(&self) -> bool {
        self.picked.is_empty()
    }

    pub fn contains(&self, id: &str) -> bool {
        self.picked.iter().any(|picked| picked == id)
    }

    /// Pick all of `ids`, or unpick them when all are picked already.
    pub fn toggle_all(&mut self, ids: &[String]) {
        match ids.iter().all(|id| self.contains(id)) {
            true => self.picked.retain(|picked| !ids.contains(picked)),
            false => {
                let missing: Vec<String> = ids
                    .iter()
                    .filter(|id| !self.contains(id))
                    .cloned()
                    .collect();
                self.picked.extend(missing);
            }
        }
    }

    pub fn clear(&mut self) {
        self.picked.clear();
    }

    /// Forget picks whose item is gone.
    pub fn keep_existing(&mut self, rows: &[Row]) {
        // Read-only rows after `Inherited` can share an id with a project row; they never
        // keep a pick alive.
        let own = rows.iter().take_while(|row| !matches!(row, Row::Inherited));
        let ids: Vec<&str> = own
            .filter_map(|row| match row {
                Row::Item(item) => Some(item.id.as_str()),
                _ => None,
            })
            .collect();
        self.picked.retain(|id| ids.contains(&id.as_str()));
    }

    /// How many of `items` are picked.
    pub fn count<'a>(&self, items: impl Iterator<Item = &'a Item>) -> usize {
        items.filter(|item| self.contains(&item.id)).count()
    }

    /// The ids to remove.
    pub fn picks(&self) -> Vec<String> {
        self.picked.clone()
    }
}
