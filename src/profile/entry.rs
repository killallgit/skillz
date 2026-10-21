//! What a profile loads, one variant per kind of thing.
//!
//! A new kind is a variant here, an arm in `validate` and `spec`, and a `provision` arm in each
//! adapter that can take it.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::model::Agent;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Entry {
    /// Skills of one `org/repo` (or git URL, or local path), installed with `npx skills`.
    Skill {
        source: String,
        names: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        agents: Vec<Agent>,
    },
    /// One plugin of a marketplace; `source` is what adds the marketplace and `marketplace` the
    /// name the agent files it under.
    Plugin {
        source: String,
        marketplace: String,
        name: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        agents: Vec<Agent>,
    },
    /// Subagent definitions (`name.md` for Claude, `name.toml` for Codex) copied from a repo or
    /// directory; `names` are file names or the agents' own names.
    Subagent {
        source: String,
        names: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        agents: Vec<Agent>,
    },
    /// A hook running `command` on `event`, for the tools matching `matcher` (all when absent).
    Hook {
        event: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        matcher: Option<String>,
        command: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        agents: Vec<Agent>,
    },
    /// An MCP server, launched by `command` or reached at `url`. Env vars and headers are
    /// secrets and cannot be part of a profile, so servers that need them are not supported.
    Mcp {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        command: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        args: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
        /// `http` (the default) or `sse`; only with a `url`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        transport: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        agents: Vec<Agent>,
    },
}

impl Entry {
    /// The entry's own agents; empty means "the profile's".
    pub fn agents(&self) -> &[Agent] {
        match self {
            Entry::Skill { agents, .. }
            | Entry::Plugin { agents, .. }
            | Entry::Mcp { agents, .. }
            | Entry::Hook { agents, .. }
            | Entry::Subagent { agents, .. } => agents,
        }
    }

    /// The same entry for exactly these agents.
    pub fn with_agents(mut self, new: Vec<Agent>) -> Entry {
        let (Entry::Skill { agents, .. }
        | Entry::Plugin { agents, .. }
        | Entry::Mcp { agents, .. }
        | Entry::Hook { agents, .. }
        | Entry::Subagent { agents, .. }) = &mut self;
        *agents = new;
        self
    }
}

impl fmt::Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Entry::Skill { source, names, .. } => {
                write!(f, "skills {} [{source}]", names.join(","))
            }
            Entry::Subagent { source, names, .. } => {
                write!(f, "subagents {} [{source}]", names.join(","))
            }
            Entry::Hook { event, command, .. } => write!(f, "hook {event} [{command}]"),
            Entry::Plugin {
                marketplace, name, ..
            } => write!(f, "plugin {name}@{marketplace}"),
            Entry::Mcp {
                name, command, url, ..
            } => {
                let target = command.as_deref().or(url.as_deref()).unwrap_or("");
                write!(f, "mcp {name} [{target}]")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_entry_can_be_given_other_agents() {
        let entry = Entry::Plugin {
            source: "o/mp".into(),
            marketplace: "mp".into(),
            name: "p".into(),
            agents: vec![],
        };

        let for_codex = entry.with_agents(vec![Agent::Codex]);

        assert_eq!(for_codex.agents(), [Agent::Codex]);
    }
}
