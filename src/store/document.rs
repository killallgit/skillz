use serde::{Deserialize, Serialize};

use crate::library::Catalog;
use crate::profile::Profile;

pub(super) const VERSION: u32 = 1;

/// Everything skillz keeps: the library of available items and the named profiles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    pub(super) version: u32,
    #[serde(default)]
    pub items: Catalog,
    #[serde(default)]
    pub profiles: Vec<Profile>,
}

impl Default for Document {
    fn default() -> Self {
        Document {
            version: VERSION,
            items: Catalog::default(),
            profiles: vec![],
        }
    }
}
