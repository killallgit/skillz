use serde::{Deserialize, Serialize};

use super::provenance::Provenance;
use crate::model::Kind;
use crate::profile::{Entry, same_thing};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemKind {
    Skill,
    Plugin,
    Subagent,
    Mcp,
    Hook,
}

impl ItemKind {
    pub fn title(self) -> &'static str {
        match self {
            ItemKind::Skill => "Skills",
            ItemKind::Plugin => "Plugins",
            ItemKind::Mcp => "MCP servers",
            ItemKind::Hook => "Hooks",
            ItemKind::Subagent => "Subagents",
        }
    }

    pub fn of_entry(entry: &Entry) -> ItemKind {
        match entry {
            Entry::Skill { .. } => ItemKind::Skill,
            Entry::Plugin { .. } => ItemKind::Plugin,
            Entry::Subagent { .. } => ItemKind::Subagent,
            Entry::Mcp { .. } => ItemKind::Mcp,
            Entry::Hook { .. } => ItemKind::Hook,
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

/// One available augmentation: an entry a profile can be given a copy of, with a display label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryItem {
    pub label: String,
    pub entry: Entry,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
}

impl LibraryItem {
    pub fn new(label: impl Into<String>, entry: Entry) -> Self {
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
    pub fn from_entry(entry: Entry) -> Self {
        LibraryItem::new(entry.to_string(), entry)
    }

    /// One item per thing `entry` names, so a skill listed with several names never overlaps a
    /// later item that shares only some of them.
    pub fn each_of(entry: Entry) -> Vec<LibraryItem> {
        let single = |names: &Vec<String>| names.len() > 1;
        match &entry {
            Entry::Skill {
                source,
                names,
                agents,
            } if single(names) => names
                .iter()
                .map(|name| Entry::Skill {
                    source: source.clone(),
                    names: vec![name.clone()],
                    agents: agents.clone(),
                })
                .map(LibraryItem::from_entry)
                .collect(),
            Entry::Subagent {
                source,
                names,
                agents,
            } if single(names) => names
                .iter()
                .map(|name| Entry::Subagent {
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
            Entry::Skill { source, .. }
            | Entry::Subagent { source, .. }
            | Entry::Plugin { source, .. } => source.clone(),
            Entry::Hook { command, .. } => command.clone(),
            Entry::Mcp { command, url, .. } => command.clone().or(url.clone()).unwrap_or_default(),
        }
    }

    pub fn kind(&self) -> ItemKind {
        ItemKind::of_entry(&self.entry)
    }

    pub fn is_same_thing_as(&self, other: &LibraryItem) -> bool {
        same_thing(&self.entry, &other.entry)
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    pub fn skill(source: &str, name: &str) -> LibraryItem {
        LibraryItem::from_entry(Entry::Skill {
            source: source.into(),
            names: vec![name.into()],
            agents: vec![],
        })
    }

    pub fn hook(command: &str) -> LibraryItem {
        LibraryItem::from_entry(Entry::Hook {
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
