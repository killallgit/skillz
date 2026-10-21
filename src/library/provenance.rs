use serde::{Deserialize, Serialize};

use crate::model::{Augmentation, Installer};

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
    pub fn of(installed: &Augmentation, recorded_at: u64) -> Self {
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
