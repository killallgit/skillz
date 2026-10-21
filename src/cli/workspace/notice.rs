//! What the workspace tells the user after an act, kept typed until the toast is built.

use std::fmt;

pub(super) enum Notice {
    Failed(String),
    UpToDate,
    /// A dry run: nothing changed.
    DryRun(usize),
    Installed(String),
    Removed(String),
    Applied(String),
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
        }
    }
}
