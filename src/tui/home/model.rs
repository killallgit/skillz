//! The data the workspace screen shows and the requests it can make.

use std::path::PathBuf;

pub struct ProjectRow {
    pub path: PathBuf,
    /// Shown in the list: the loaded profile and a short path.
    pub label: String,
    pub missing: bool,
    /// Detail lines: the loaded profile and what it added.
    pub detail: Vec<String>,
    pub loaded: bool,
}

pub struct ProfileRow {
    pub name: String,
    /// Detail lines: one per entry, or why the file is unreadable.
    pub detail: Vec<String>,
    pub broken: bool,
}

#[derive(Default)]
pub struct Model {
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
