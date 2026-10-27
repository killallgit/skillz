//! What the workspace screen exchanges with the flow that drives it: the `Act` the user asked
//! for, the `Model` the tabs show, the `Synced` outcomes of background syncs and the `Screen`
//! the flow feeds. Things are named by id, entry or profile name, so an act still means the
//! same if the lists changed while it was in flight.

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use crate::app::library::Catalog;
use crate::app::profile::Profile;
use skillz_harness::Spec;
use skillz_harness::model::{Agent, Installed, Scope};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    Quit,
    /// Bring the installed items with these ids in `scope` to their latest version, as one plan.
    Update {
        scope: Scope,
        ids: Vec<String>,
    },
    /// Install these entries and remove the installed items with these ids, as one plan, in
    /// `scope`.
    Change {
        scope: Scope,
        install: Vec<Spec>,
        remove: Vec<String>,
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
    /// The coding tools an install can be made for.
    pub agents: Vec<Agent>,
    pub library: Catalog,
    /// The readable profiles, with what they extend already merged in.
    pub profiles: Vec<Profile>,
    /// The profile last applied to the project.
    pub applied: Option<String>,
    /// The profile last applied at user level.
    pub applied_global: Option<String>,
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
