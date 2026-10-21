//! What the tabs show: a snapshot of the project, the library and the profiles, read again
//! after every act.

use crate::library::Catalog;
use crate::model::Augmentation;
use crate::profile::Profile;

#[derive(Default)]
pub struct Model {
    /// The project directory as shown, home abbreviated.
    pub dir: String,
    /// What the project has installed, read from the agents' own config.
    pub installed: Vec<Augmentation>,
    /// What the user has installed outside any project.
    pub global: Vec<Augmentation>,
    pub library: Catalog,
    /// The readable profiles, with what they extend already merged in.
    pub profiles: Vec<Profile>,
    /// Shown as a toast: what the last act did or why it failed.
    pub notice: Option<String>,
}
