use std::fmt;

use serde::{Deserialize, Serialize};

use crate::action::Action;
use crate::hook_text::HookEntry;

/// A coding agent whose harness we manage. Add a variant + an adapter in `agents/` to extend.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    clap::ValueEnum,
    Serialize,
    Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    Claude,
    Codex,
    Cursor,
}

impl Agent {
    /// The id the `skills` CLI (`npx skills add -a <id>`) uses for this agent.
    pub fn npx_id(self) -> &'static str {
        match self {
            Agent::Claude => "claude-code",
            Agent::Codex => "codex",
            Agent::Cursor => "cursor",
        }
    }
}

/// The kind of harness augmentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Marketplace,
    Plugin,
    Skill,
    Hook,
    Mcp,
    Subagent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Global,
    Project,
}

/// What put an item there, so removal can use the same tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
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

/// Where an installed item was fetched from and which revision, as its installer recorded it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Origin {
    /// A repository URL or path the item can be fetched from again.
    pub link: Option<String>,
    /// A plugin version or a content hash.
    pub version: Option<String>,
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
    pub origin: Origin,
    /// Display-only contents (a plugin's skills, a hook group's commands).
    pub children: Vec<String>,
    /// The handlers of a hook group as data; `children` only shows them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hooks: Vec<HookEntry>,
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

/// `name@marketplace` -> (`name`, `marketplace`); a bare name has an empty marketplace.
pub fn split_plugin_id(id: &str) -> (&str, &str) {
    id.rsplit_once('@').unwrap_or((id, ""))
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
            Kind::Mcp => "mcp",
            Kind::Subagent => "subagent",
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
            hooks: Vec::new(),
            id: "skill:a".into(),
            name: "a".into(),
            kind: Kind::Skill,
            agents: vec![Agent::Claude],
            scope: Scope::Global,
            group: "g".into(),
            source: source.map(Into::into),
            installer: Installer::Local,
            origin: Origin::default(),
            children: vec![],
            removal: vec![],
        }
    }

    #[test]
    fn every_agent_has_a_distinct_npx_id() {
        let mut ids = [Agent::Claude, Agent::Codex, Agent::Cursor].map(Agent::npx_id);
        ids.sort();

        let distinct = ids.windows(2).all(|w| w[0] != w[1]);

        // A shared id would make `npx skills -a` act on the wrong agent.
        assert!(distinct && ids.iter().all(|id| !id.is_empty()));
    }

    #[test]
    fn plugin_ids_split_at_the_last_at_sign() {
        assert_eq!(split_plugin_id("tdd@mp"), ("tdd", "mp"));
        assert_eq!(split_plugin_id("bare"), ("bare", ""));
        assert_eq!(split_plugin_id("a@b@c"), ("a@b", "c"));
    }

    #[test]
    fn label_shows_source_only_when_asked_and_it_differs_from_name() {
        let other = item(Some("org/repo"));
        assert_ne!(other.label(true), other.label(false));
        for same in [item(None), item(Some("a"))] {
            assert_eq!(same.label(true), same.label(false));
        }
    }
}
