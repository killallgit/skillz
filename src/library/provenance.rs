use serde::{Deserialize, Serialize};

use crate::model::{Installed, Installer};

/// How a library item got onto this machine, recorded when it was installed or imported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// A repository URL or path it can be fetched from again.
    pub link: Option<String>,
    pub engine: Installer,
    /// A plugin version or a content hash. Recorded for reference only: none of the install
    /// tools can fetch a given revision.
    pub version: Option<String>,
    /// Seconds since the Unix epoch.
    pub recorded_at: u64,
    /// Found already installed by a scan, not put there by skillz.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub detected: bool,
}

impl Provenance {
    pub fn of(installed: &Installed, recorded_at: u64) -> Self {
        Provenance {
            link: installed.origin.link.clone(),
            engine: installed.installer,
            version: installed.origin.version.clone(),
            recorded_at,
            detected: false,
        }
    }

    pub fn detected(self) -> Self {
        Provenance {
            detected: true,
            ..self
        }
    }
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// How an item came to be in the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Cue {
    /// Typed in by hand.
    Manual,
    /// Found already installed by a scan.
    Detected,
    /// Recorded when skillz installed it.
    Skillz,
}

impl Cue {
    pub fn of(provenance: Option<&Provenance>) -> Cue {
        match provenance {
            None => Cue::Manual,
            Some(p) if p.detected => Cue::Detected,
            Some(_) => Cue::Skillz,
        }
    }
}
