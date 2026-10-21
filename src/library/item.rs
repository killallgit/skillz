use serde::{Deserialize, Serialize};

use crate::profile::{Entry, same_thing};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemKind {
    Skill,
    Plugin,
    Subagent,
    Mcp,
    Hook,
}

/// One available augmentation: an entry a profile can be given a copy of, with a display label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryItem {
    pub label: String,
    pub entry: Entry,
}

impl LibraryItem {
    pub fn new(label: impl Into<String>, entry: Entry) -> Self {
        LibraryItem {
            label: label.into(),
            entry,
        }
    }

    /// An item labelled by how its entry describes itself.
    pub fn from_entry(entry: Entry) -> Self {
        LibraryItem::new(entry.to_string(), entry)
    }

    pub fn kind(&self) -> ItemKind {
        match self.entry {
            Entry::Skill { .. } => ItemKind::Skill,
            Entry::Plugin { .. } => ItemKind::Plugin,
            Entry::Subagent { .. } => ItemKind::Subagent,
            Entry::Mcp { .. } => ItemKind::Mcp,
            Entry::Hook { .. } => ItemKind::Hook,
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
