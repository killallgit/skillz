//! What the user asked the workspace to do. Things are named by id, entry or profile name, so
//! a request still means the same if the lists changed while it was in flight.

use crate::model::Scope;
use crate::profile::Entry;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    Quit,
    /// Install these entries and remove the installed items with these ids in `scope`, as one
    /// plan.
    Change {
        scope: Scope,
        install: Vec<Entry>,
        remove: Vec<String>,
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
    /// Add the item a one-line entry describes to the library.
    LibraryAdd {
        spec: String,
    },
    /// Add what the project has installed to the library.
    LibraryImport,
    LibraryRemove(Entry),
    /// Make the project hold exactly what the profile has, removing the rest.
    ApplyExact {
        profile: String,
    },
    NewProfile {
        name: String,
    },
    /// Store what the project has installed as a new profile.
    SaveCurrent {
        name: String,
    },
    /// Replace the profile's entries.
    SetEntries {
        profile: String,
        entries: Vec<Entry>,
    },
    DeleteProfile {
        name: String,
    },
}
