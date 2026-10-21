//! The data the workspace screen shows and the requests it can make.

use std::path::PathBuf;

pub struct ProjectRow {
    pub path: PathBuf,
    /// The profile loaded into the project, if any.
    pub profile: Option<String>,
    /// The path as shown in the list, home abbreviated.
    pub short: String,
    pub missing: bool,
    /// Detail lines: the path, then the loaded profile's entries.
    pub detail: Vec<String>,
}

impl ProjectRow {
    pub fn loaded(&self) -> bool {
        self.profile.is_some()
    }

    /// What the filter matches against: the loaded profile and the short path.
    pub fn search_text(&self) -> String {
        format!("{} {}", self.profile.as_deref().unwrap_or(""), self.short)
    }
}

pub struct ProfileRow {
    pub name: String,
    /// Detail lines: one per entry, or why the file is unreadable.
    pub detail: Vec<String>,
    pub broken: bool,
}

/// The project the workspace was opened in.
pub struct Current {
    pub short: String,
    pub profile: Option<String>,
}

#[derive(Default)]
pub struct Model {
    pub current: Option<Current>,
    pub projects: Vec<ProjectRow>,
    pub profiles: Vec<ProfileRow>,
    /// Shown as a toast until the next key: what the last action did or why it failed.
    pub notice: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Focus {
    #[default]
    Projects,
    Profiles,
}

impl Focus {
    pub fn other(self) -> Self {
        match self {
            Focus::Projects => Focus::Profiles,
            Focus::Profiles => Focus::Projects,
        }
    }
}

/// Where the cursors were, kept across screens so a refresh does not jump back to the top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pos {
    pub focus: Focus,
    pub project: usize,
    pub profile: usize,
}

/// What the user asked for. Indices are into the `Model`'s rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    Quit,
    Apply {
        project: usize,
        profile: usize,
    },
    Unload {
        project: usize,
    },
    AddProject(String),
    ForgetProject(usize),
    NewProfile(String),
    SaveProfile {
        name: String,
        project: usize,
    },
    DeleteProfile(usize),
    /// Offer the project's installed items for the profile.
    AddInstalled {
        profile: usize,
        project: usize,
    },
    /// Offer the profile's entries for removal.
    RemoveEntries {
        profile: usize,
    },
    AddTyped {
        profile: usize,
        spec: String,
    },
}
