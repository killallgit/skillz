//! What the workspace tells the user after an act, kept typed until the toast is built.

use std::fmt;

use crate::snapshot::Skipped;

#[derive(Debug)]
pub(super) enum Notice {
    Failed(String),
    UpToDate,
    /// A dry run: nothing changed.
    DryRun(usize),
    Installed(String),
    Removed(String),
    Applied(String),
    LibraryAdded(String),
    LibraryRemoved(String),
    /// How many items the library gained, and what was left out.
    Imported {
        added: usize,
        skipped: Vec<Skipped>,
    },
    Created(String),
    /// A profile saved from the project, with the installed items it could not hold.
    Saved {
        name: String,
        skipped: Vec<String>,
    },
    Updated(String),
    Deleted(String),
}

impl fmt::Display for Notice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Notice::Failed(message) => f.write_str(message),
            Notice::UpToDate => f.write_str("already up to date"),
            Notice::DryRun(steps) => write!(f, "dry run: {steps} steps, nothing changed"),
            Notice::Installed(what) => write!(f, "installed {what}"),
            Notice::Removed(what) => write!(f, "removed {what}"),
            Notice::Applied(profile) => write!(f, "applied {profile}"),
            Notice::LibraryAdded(what) => write!(f, "added {what} to the library"),
            Notice::LibraryRemoved(what) => write!(f, "removed {what} from the library"),
            Notice::Imported { added, skipped } => write_imported(f, *added, skipped),
            Notice::Created(name) => write!(f, "created {name}"),
            Notice::Saved { name, skipped } if skipped.is_empty() => write!(f, "saved {name}"),
            Notice::Saved { name, skipped } => {
                write!(f, "saved {name}; not saved: {}", skipped.join(", "))
            }
            Notice::Updated(name) => write!(f, "updated {name}"),
            Notice::Deleted(name) => write!(f, "deleted {name}"),
        }
    }
}

fn write_imported(f: &mut fmt::Formatter<'_>, added: usize, skipped: &[Skipped]) -> fmt::Result {
    write!(f, "imported {added}")?;
    if skipped.is_empty() {
        return Ok(());
    }
    let left_out: Vec<String> = skipped
        .iter()
        .map(|s| format!("{} ({})", s.label, s.reason))
        .collect();
    write!(f, ", skipped {}: {}", skipped.len(), left_out.join("; "))
}
