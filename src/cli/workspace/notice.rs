//! What the workspace tells the user after an act, kept typed until the toast is built.

use std::fmt;

#[derive(Debug)]
pub(super) enum Notice {
    Failed(String),
    UpToDate,
    /// A dry run: nothing changed.
    DryRun(usize),
    Installed(String),
    Removed(String),
    Applied(String),
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
