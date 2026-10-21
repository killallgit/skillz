//! Whether library items are installed right now, in the project and at user level.

use crate::model::Augmentation;
use crate::profile::{Entry, same_thing};
use crate::snapshot::snapshot;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Presence {
    pub project: bool,
    pub global: bool,
}

/// What the project and the user have installed, as the entries that would install it.
pub struct Held {
    project: Vec<Entry>,
    global: Vec<Entry>,
}

impl Held {
    pub fn of(project: &[Augmentation], global: &[Augmentation]) -> Self {
        Held {
            project: entries_of(project),
            global: entries_of(global),
        }
    }

    pub fn presence(&self, entry: &Entry) -> Presence {
        Presence {
            project: holds(&self.project, entry),
            global: holds(&self.global, entry),
        }
    }
}

fn entries_of(installed: &[Augmentation]) -> Vec<Entry> {
    snapshot("installed", installed).profile.entries
}

fn holds(installed: &[Entry], entry: &Entry) -> bool {
    installed.iter().any(|have| same_thing(have, entry))
}
