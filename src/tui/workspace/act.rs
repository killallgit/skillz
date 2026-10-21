//! What the user asked the workspace to do. Things are named by id, entry or profile name, so
//! a request still means the same if the lists changed while it was in flight.

use crate::profile::Entry;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    Quit,
    /// Install what the entry describes in the project.
    Install(Entry),
    /// Remove the installed item with this id from the project.
    Remove {
        id: String,
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
}
