use serde::{Deserialize, Serialize};

use super::provenance::Provenance;
use crate::profile::same_thing;
use skillz_harness::Spec;
use skillz_harness::model::Kind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemKind {
    Skill,
    Plugin,
    Subagent,
    Mcp,
    Hook,
}

impl ItemKind {
    /// What one of this kind is called: `skill`, `MCP server`.
    pub fn noun(self) -> &'static str {
        match self {
            ItemKind::Skill => "skill",
            ItemKind::Plugin => "plugin",
            ItemKind::Mcp => "MCP server",
            ItemKind::Hook => "hook",
            ItemKind::Subagent => "subagent",
        }
    }

    /// What several of this kind are called: `skills`, `MCP servers`.
    pub fn plural(self) -> &'static str {
        match self {
            ItemKind::Skill => "skills",
            ItemKind::Plugin => "plugins",
            ItemKind::Mcp => "MCP servers",
            ItemKind::Hook => "hooks",
            ItemKind::Subagent => "agents",
        }
    }

    pub fn of_entry(entry: &Spec) -> ItemKind {
        match entry {
            Spec::Skill { .. } => ItemKind::Skill,
            Spec::Plugin { .. } => ItemKind::Plugin,
            Spec::Subagent { .. } => ItemKind::Subagent,
            Spec::Mcp { .. } => ItemKind::Mcp,
            Spec::Hook { .. } => ItemKind::Hook,
        }
    }

    /// The library kind of an installed item's kind; a marketplace is not an item of its own.
    pub fn of(kind: Kind) -> Option<ItemKind> {
        match kind {
            Kind::Skill => Some(ItemKind::Skill),
            Kind::Plugin => Some(ItemKind::Plugin),
            Kind::Subagent => Some(ItemKind::Subagent),
            Kind::Mcp => Some(ItemKind::Mcp),
            Kind::Hook => Some(ItemKind::Hook),
            Kind::Marketplace => None,
        }
    }
}

/// One available item: a spec a profile can be given a copy of, with a display label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryItem {
    pub label: String,
    pub entry: Spec,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
}

impl LibraryItem {
    pub fn new(label: impl Into<String>, entry: Spec) -> Self {
        LibraryItem {
            label: label.into(),
            entry,
            provenance: None,
        }
    }

    pub fn with_provenance(self, provenance: Provenance) -> Self {
        LibraryItem {
            provenance: Some(provenance),
            ..self
        }
    }

    /// An item labelled by how its entry describes itself.
    pub fn from_entry(entry: Spec) -> Self {
        LibraryItem::new(entry.to_string(), entry)
    }

    /// One item per thing `entry` names, so a skill listed with several names never overlaps a
    /// later item that shares only some of them.
    pub fn each_of(entry: Spec) -> Vec<LibraryItem> {
        let single = |names: &Vec<String>| names.len() > 1;
        match &entry {
            Spec::Skill {
                source,
                names,
                agents,
            } if single(names) => names
                .iter()
                .map(|name| Spec::Skill {
                    source: source.clone(),
                    names: vec![name.clone()],
                    agents: agents.clone(),
                })
                .map(LibraryItem::from_entry)
                .collect(),
            Spec::Subagent {
                source,
                names,
                agents,
            } if single(names) => names
                .iter()
                .map(|name| Spec::Subagent {
                    source: source.clone(),
                    names: vec![name.clone()],
                    agents: agents.clone(),
                })
                .map(LibraryItem::from_entry)
                .collect(),
            _ => vec![LibraryItem::from_entry(entry)],
        }
    }

    /// Where it comes from or what it runs.
    pub fn source(&self) -> String {
        match &self.entry {
            Spec::Skill { source, .. }
            | Spec::Subagent { source, .. }
            | Spec::Plugin { source, .. } => source.clone(),
            Spec::Hook { command, .. } => command.clone(),
            Spec::Mcp { command, url, .. } => command.clone().or(url.clone()).unwrap_or_default(),
        }
    }

    pub fn kind(&self) -> ItemKind {
        ItemKind::of_entry(&self.entry)
    }

    /// The label inside its package: the thing's own name, unless the item was given a label.
    pub fn short_label(&self) -> String {
        match self.label == self.entry.to_string() {
            true => self.entry.name(),
            false => self.label.clone(),
        }
    }

    pub fn is_same_thing_as(&self, other: &LibraryItem) -> bool {
        same_thing(&self.entry, &other.entry)
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    pub fn skill(source: &str, name: &str) -> LibraryItem {
        LibraryItem::from_entry(Spec::Skill {
            source: source.into(),
            names: vec![name.into()],
            agents: vec![],
        })
    }

    pub fn hook(command: &str) -> LibraryItem {
        LibraryItem::from_entry(Spec::Hook {
            event: "Stop".into(),
            matcher: None,
            command: command.into(),
            agents: vec![],
        })
    }

    #[test]
    fn an_installed_marketplace_is_no_library_kind() {
        assert_eq!(ItemKind::of(Kind::Mcp), Some(ItemKind::Mcp));
        assert_eq!(ItemKind::of(Kind::Marketplace), None);
    }

    #[test]
    fn a_skill_comes_from_its_source_and_a_hook_runs_its_command() {
        assert_eq!(skill("o/r", "a").source(), "o/r");
        assert_eq!(hook("notify").source(), "notify");
    }

    #[test]
    fn kind_follows_the_entry() {
        assert_eq!(skill("o/r", "a").kind(), ItemKind::Skill);
        assert_eq!(hook("x").kind(), ItemKind::Hook);
    }

    #[test]
    fn items_with_the_same_entry_identity_are_the_same_thing_whatever_their_label() {
        let relabelled = LibraryItem::new("other", skill("o/r", "a").entry);

        assert!(skill("o/r", "a").is_same_thing_as(&relabelled));
        assert!(!skill("o/r", "a").is_same_thing_as(&skill("o/r", "b")));
    }
}
