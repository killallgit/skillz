//! The data the workspace screen shows and the requests it can make.

use std::path::PathBuf;

use crate::tui::theme::State;

/// The kinds of profile entry, which a detail pane groups and tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntryKind {
    Skill,
    Plugin,
    Subagent,
    Hook,
    Mcp,
}

impl EntryKind {
    pub fn tag(self) -> &'static str {
        match self {
            EntryKind::Skill => "skill",
            EntryKind::Plugin => "plugin",
            EntryKind::Subagent => "agent",
            EntryKind::Hook => "hook",
            EntryKind::Mcp => "mcp",
        }
    }
}

/// One line of a detail pane. Styles are chosen when it is drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetailLine {
    Heading(String),
    Text(String),
    /// An entry under its kind tag.
    Entry(EntryKind, String),
    /// Something an entry added, under it.
    Child(String),
    Status(State, String),
    Blank,
}

pub struct ProjectRow {
    pub path: PathBuf,
    /// The profile loaded into the project, if any.
    pub profile: Option<String>,
    /// The path as shown in the list, home abbreviated.
    pub short: String,
    pub missing: bool,
    /// Detail lines: the path, how it compares with its profile, then the loaded entries.
    pub detail: Vec<DetailLine>,
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
    pub detail: Vec<DetailLine>,
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

/// What the user asked for. Projects are named by path and profiles by name, so a request still
/// means the same thing if the lists changed while it was in flight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    Quit,
    Apply {
        project: PathBuf,
        profile: String,
    },
    NewProfile(String),
    SaveProfile {
        name: String,
        project: PathBuf,
    },
    DeleteProfile(String),
    /// Offer the project's installed items for the profile.
    AddInstalled {
        profile: String,
        project: PathBuf,
    },
    /// Offer the profile's entries for removal.
    RemoveEntries {
        profile: String,
    },
    AddTyped {
        profile: String,
        spec: String,
    },
}
