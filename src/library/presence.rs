//! Whether library items are installed right now, in the project and at user level.

use std::collections::HashSet;

use crate::model::Augmentation;
use crate::profile::{Entry, Key, installed_keys, keys};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Presence {
    pub project: bool,
    pub global: bool,
}

/// What the project and the user have installed, as identities independent of reinstallability.
pub struct Held {
    project: HashSet<Key>,
    global: HashSet<Key>,
}

impl Held {
    pub fn of(project: &[Augmentation], global: &[Augmentation]) -> Self {
        Held {
            project: keys_of(project),
            global: keys_of(global),
        }
    }

    pub fn presence(&self, entry: &Entry) -> Presence {
        Presence {
            project: holds(&self.project, entry),
            global: holds(&self.global, entry),
        }
    }
}

fn keys_of(installed: &[Augmentation]) -> HashSet<Key> {
    installed
        .iter()
        .flat_map(Augmentation::parts)
        .flat_map(installed_keys)
        .collect()
}

fn holds(installed: &HashSet<Key>, entry: &Entry) -> bool {
    keys(entry).iter().any(|key| installed.contains(key))
}
