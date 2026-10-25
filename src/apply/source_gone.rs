use std::fmt;

use crate::stale::{Probe, missing_source};
use skillz_harness::Spec;

/// An entry cannot be installed because the local folder it names is gone, most likely moved.
#[derive(Debug, PartialEq, Eq)]
pub struct SourceGone {
    pub entry: String,
    pub path: String,
}

impl SourceGone {
    /// Why `entry` cannot be installed, when its local source is gone.
    pub fn of(entry: &Spec, probe: &dyn Probe) -> Option<Self> {
        missing_source(entry, probe).map(|path| SourceGone {
            entry: entry.to_string(),
            path: path.to_string(),
        })
    }
}

impl fmt::Display for SourceGone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} cannot be installed: {} does not exist; if the folder moved, run `skillz sync` to follow it",
            self.entry, self.path
        )
    }
}

impl std::error::Error for SourceGone {}
