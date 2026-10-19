use std::fmt;

use crate::action::Action;

/// A coding agent whose harness we manage. Add a variant + an adapter in `agents/` to extend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, clap::ValueEnum)]
pub enum Agent {
    Claude,
    Codex,
    Cursor,
}

/// The kind of harness augmentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Marketplace,
    Plugin,
    Skill,
    Hook,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Scope {
    Global,
    Project,
}

/// One removable thing installed into an agent harness.
///
/// Discovery builds these with `removal` already planned, so uninstalling is just
/// executing the concatenated plans of the selected items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Augmentation {
    /// Unique, stable across runs. Usually `<kind>:<path or config key>`.
    pub id: String,
    pub name: String,
    pub kind: Kind,
    /// Every agent that sees this item (a shared skill dir can serve several).
    pub agents: Vec<Agent>,
    pub scope: Scope,
    /// Tree grouping label: marketplace name, skills source repo, or "hooks".
    pub group: String,
    /// Where it came from, for display (e.g. `mattpocock/skills`).
    pub source: Option<String>,
    /// Display-only contents (a plugin's skills, a hook group's commands).
    pub children: Vec<String>,
    /// Ordered steps that remove this item completely.
    pub removal: Vec<Action>,
}

impl fmt::Display for Agent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
            Agent::Cursor => "cursor",
        })
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Kind::Marketplace => "marketplace",
            Kind::Plugin => "plugin",
            Kind::Skill => "skill",
            Kind::Hook => "hook",
        })
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Scope::Global => "global",
            Scope::Project => "project",
        })
    }
}
