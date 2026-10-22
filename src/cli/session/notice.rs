//! What the workspace tells the user after an act, kept typed until the toast is built.

use std::fmt;

#[derive(Debug)]
pub(in crate::cli) enum Notice {
    Failed(String),
    UpToDate,
    /// A dry run: nothing changed.
    DryRun(usize),
    /// How many items a batch installed and removed.
    Changed {
        installed: usize,
        removed: usize,
    },
    /// How many items were brought to their latest version.
    ItemsUpdated(usize),
    Applied(String),
    LibraryAdded(String),
    LibraryRemoved(String),
    /// How many items the library gained from what is installed, how many moved folders it
    /// followed, and how many installed items were left out for needing secrets.
    Synced {
        added: usize,
        moved: usize,
        secrets: usize,
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
            Notice::Changed { installed, removed } => {
                write!(f, "installed {installed}, removed {removed}")
            }
            Notice::ItemsUpdated(count) => write!(f, "updated {count}"),
            Notice::Applied(profile) => write!(f, "applied {profile}"),
            Notice::LibraryAdded(what) => write!(f, "added {what} to the library"),
            Notice::LibraryRemoved(what) => write!(f, "removed {what} from the library"),
            Notice::Synced {
                added,
                moved,
                secrets,
            } => {
                write!(f, "library: {added} new")?;
                if *moved > 0 {
                    write!(f, ", moved: {moved}")?;
                }
                if *secrets > 0 {
                    write!(f, ", {secrets} left out for needing secrets")?;
                }
                Ok(())
            }
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
