//! What the workspace screen exchanges with the flow that drives it: the `Act` the user asked
//! for, the `Model` the tabs show, the `Synced` outcomes of background syncs and the `Screen`
//! the flow feeds. Things are named by id, entry or profile name, so an act still means the
//! same if the lists changed while it was in flight.

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use crate::app::library::Catalog;
use skillz_harness::Spec;
use skillz_harness::model::{Agent, Installed, Scope};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    Quit,
    /// Install these entries and remove the installed items with these ids, as one plan, in
    /// `scope`.
    Change {
        scope: Scope,
        install: Vec<Spec>,
        remove: Vec<String>,
    },
}

/// What the list shows: a snapshot of the project, the user's installs and the library, read
/// again after every act.
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
    /// Show the outcome of each library sync that arrives on `done`.
    fn watch_syncs(&mut self, done: Receiver<Synced>);
}

/// A screen shared with whatever redraws it while a change runs.
pub type SharedScreen = Arc<Mutex<dyn Screen>>;
