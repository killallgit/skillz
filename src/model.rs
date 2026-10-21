use std::fmt;

use serde::Serialize;

use crate::action::Action;

/// A coding agent whose harness we manage. Add a variant + an adapter in `agents/` to extend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, clap::ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    Claude,
    Codex,
    Cursor,
}

/// The kind of harness augmentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Marketplace,
    Plugin,
    Skill,
    Hook,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Global,
    Project,
}

/// What put an item there, so removal can use the same tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Installer {
    /// `npx skills add`: the skill has an entry in the scope's `.skill-lock.json`.
    Npx,
    /// `claude plugin install` / `claude plugin marketplace add`.
    ClaudeCli,
    /// `codex plugin add` / `codex plugin marketplace add`.
    CodexCli,
    /// No tool recorded it: a skill or hook put there by hand or copied from a local path.
    Local,
}

/// One removable thing installed into an agent harness.
///
/// Discovery builds these with `removal` already planned, so uninstalling is just
/// executing the concatenated plans of the selected items.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
    pub installer: Installer,
    /// Display-only contents (a plugin's skills, a hook group's commands).
    pub children: Vec<String>,
    /// Steps that remove this item completely. `execute` commits every `Edit` first (all or
    /// none), then runs each `RemovePath` in plan order.
    pub removal: Vec<Action>,
}

/// Prefix of a plugin child that names one of its skills (`skill: <name>`).
pub const SKILL_CHILD: &str = "skill: ";

impl Augmentation {
    /// The skill names among a plugin's children.
    pub fn plugin_skills(&self) -> impl Iterator<Item = &str> {
        self.children
            .iter()
            .filter_map(|c| c.strip_prefix(SKILL_CHILD))
    }

    /// `name [source]  (kind · agents · scope)`. Tree items pass `false`: their group
    /// header already shows the source.
    pub fn label(&self, with_source: bool) -> String {
        let agents: Vec<String> = self.agents.iter().map(Agent::to_string).collect();
        let source = self
            .source
            .as_deref()
            .filter(|s| with_source && *s != self.name)
            .map(|s| format!(" [{s}]"))
            .unwrap_or_default();
        format!(
            "{}{source}  ({} · {} · {})",
            self.name,
            self.kind,
            agents.join(","),
            self.scope
        )
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn item(source: Option<&str>) -> Augmentation {
        Augmentation {
            id: "skill:a".into(),
            name: "a".into(),
            kind: Kind::Skill,
            agents: vec![Agent::Claude],
            scope: Scope::Global,
            group: "g".into(),
            source: source.map(Into::into),
            installer: Installer::Local,
            children: vec![],
            removal: vec![],
        }
    }

    #[test]
    fn label_shows_source_only_when_asked_and_it_differs_from_name() {
        let other = item(Some("org/repo"));
        assert_ne!(other.label(true), other.label(false));
        assert!(other.label(true).contains("org/repo"));
        for same in [item(None), item(Some("a"))] {
            assert_eq!(same.label(true), same.label(false));
        }
    }
}
