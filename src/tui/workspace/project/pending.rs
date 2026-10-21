//! The changes picked but not yet applied. An item is identified by its state, so a pick
//! survives a reload for as long as the item is still there in that state.

use super::rows::{Item, Row, State};
use crate::profile::Entry;

#[derive(Debug, Clone, Default)]
pub(super) struct Pending {
    picked: Vec<State>,
}

/// How many items of a section are picked to install and to remove.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct Counts {
    pub install: usize,
    pub remove: usize,
}

impl Pending {
    pub fn is_empty(&self) -> bool {
        self.picked.is_empty()
    }

    pub fn contains(&self, state: &State) -> bool {
        self.picked.contains(state)
    }

    pub fn toggle(&mut self, state: &State) {
        match self.picked.iter().position(|picked| picked == state) {
            Some(at) => {
                self.picked.remove(at);
            }
            None => self.picked.push(state.clone()),
        }
    }

    pub fn clear(&mut self) {
        self.picked.clear();
    }

    /// Forget picks whose item is gone or has changed state.
    pub fn keep_existing(&mut self, rows: &[Row]) {
        self.picked.retain(|state| {
            rows.iter()
                .any(|row| matches!(row, Row::Item(item) if item.state == *state))
        });
    }

    pub fn counts<'a>(&self, items: impl Iterator<Item = &'a Item>) -> Counts {
        items
            .filter(|item| self.contains(&item.state))
            .fold(Counts::default(), |counts, item| match item.state {
                State::Available(_) => Counts {
                    install: counts.install + 1,
                    ..counts
                },
                State::Installed(_) => Counts {
                    remove: counts.remove + 1,
                    ..counts
                },
                State::Held { .. } => counts,
            })
    }

    /// The entries to install and the ids to remove.
    pub fn picks(&self) -> (Vec<Entry>, Vec<String>) {
        self.picked
            .iter()
            .fold((vec![], vec![]), |(mut install, mut remove), state| {
                match state {
                    State::Available(entry) => install.push(entry.clone()),
                    State::Installed(id) => remove.push(id.clone()),
                    State::Held { .. } => {}
                }
                (install, remove)
            })
    }
}
