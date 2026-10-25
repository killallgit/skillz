//! What the workspace screen exchanges with the flow that drives it: the `Act` the user asked
//! for, the `Model` the tabs show, the `Synced` outcomes of background syncs and the `Screen`
//! the flow feeds. Things are named by id, entry or profile name, so an act still means the
//! same if the lists changed while it was in flight.

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use crate::app::library::Catalog;
use crate::app::profile::Profile;
use skillz_harness::Spec;
use skillz_harness::model::{Installed, Scope};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    Quit,
    /// Remove the installed items with these ids in `scope`, as one plan.
    Remove {
        scope: Scope,
        ids: Vec<String>,
    },
    /// Bring the installed items with these ids in `scope` to their latest version, as one plan.
    Update {
        scope: Scope,
        ids: Vec<String>,
    },
    /// Install what the profile has that the project lacks.
    Apply {
        profile: String,
    },
    /// Install what `source` (`owner/repo`, a git URL or a path) offers each agent in `scope`.
    InstallSource {
        scope: Scope,
        source: String,
    },
    /// Add the item a one-line entry describes to the library.
    LibraryAdd {
        spec: String,
    },
    /// Add what is installed, in the project and at user level, to the library.
    LibraryImport,
    LibraryRemove(Spec),
    /// Make the project hold exactly what the profile has, removing the rest.
    ApplyExact {
        profile: String,
    },
    /// Make the project hold these entries of its profile and not the rest of it: install the
    /// entries, remove the installed items with these ids.
    Subset {
        install: Vec<Spec>,
        remove: Vec<String>,
    },
    /// Make a profile of this name holding these entries.
    CreateProfile {
        name: String,
        entries: Vec<Spec>,
    },
    /// Store what the project has installed as a new profile.
    SaveCurrent {
        name: String,
    },
    /// Replace the profile's entries.
    SetEntries {
        profile: String,
        entries: Vec<Spec>,
    },
    DeleteProfile {
        name: String,
    },
}

/// What the tabs show: a snapshot of the project, the library and the profiles, read again
/// after every act.
#[derive(Default)]
pub struct Model {
    /// The project directory as shown, home abbreviated.
    pub dir: String,
    /// What the project has installed, read from the agents' own config.
    pub installed: Vec<Installed>,
    /// What the user has installed outside any project.
    pub global: Vec<Installed>,
    pub library: Catalog,
    /// The readable profiles, with what they extend already merged in.
    pub profiles: Vec<Profile>,
    /// The profile last applied to the project.
    pub applied: Option<String>,
    /// Shown as a toast: what the last act did or why it failed.
    pub notice: Option<String>,
}

/// The outcome of one background sync: the library as it now is, when that could be read, and
/// what to toast, when there is anything worth saying.
#[derive(Debug, Default)]
pub struct Synced {
    pub library: Option<Catalog>,
    pub notice: Option<String>,
}

/// The workspace as the flow sees it: fed snapshots and background results between acts.
pub trait Screen {
    /// Show the new snapshot and toast what the last act reported.
    fn load(&mut self, model: Model);
    /// Mark the items whose ids arrive on `found` as having an update.
    fn watch_updates(&mut self, found: Receiver<String>);
    /// Show the outcome of each library sync that arrives on `done`.
    fn watch_syncs(&mut self, done: Receiver<Synced>);
    /// Drop the marks of items that were just updated.
    fn forget_updates(&mut self, ids: &[String]);
}

/// A screen shared with whatever redraws it while a change runs.
pub type SharedScreen = Arc<Mutex<dyn Screen>>;
