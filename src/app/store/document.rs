use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::app::library::Catalog;
use crate::app::profile::Profile;

pub(super) const VERSION: u32 = 1;

/// Everything skillz keeps: the library of available items and the named profiles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    pub(super) version: u32,
    #[serde(default)]
    pub items: Catalog,
    #[serde(default)]
    pub profiles: Vec<Profile>,
    /// The profile last applied to each project directory. The directory's own config stays the
    /// truth about what is installed; this only remembers which profile it was brought to.
    #[serde(default)]
    pub applied: BTreeMap<String, String>,
}

impl Default for Document {
    fn default() -> Self {
        Document {
            version: VERSION,
            items: Catalog::default(),
            profiles: vec![],
            applied: BTreeMap::new(),
        }
    }
}
