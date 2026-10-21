//! What makes two entries "the same thing", so a profile that extends another can replace its
//! entries instead of repeating them.
//!
//! A skill or subagent entry names several things, so it has one key per name. A child entry
//! replaces a parent entry holding the same key; a parent entry that only partly overlaps keeps
//! its other names, and disappears when none are left. The agents of an entry are not part of its
//! identity: the child's agents win along with the entry.

use std::collections::HashSet;

use super::Entry;
use crate::model::Agent;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    Skill(String, String),
    Subagent(String, String),
    Plugin(String, String),
    Mcp(String),
    Hook(String, Option<String>, String),
}

/// `base` without what `top` replaces, followed by `top`.
pub fn overlay(base: Vec<Entry>, top: &[Entry]) -> Vec<Entry> {
    let taken: HashSet<Key> = top.iter().flat_map(keys).collect();
    base.into_iter()
        .filter_map(|entry| without(entry, &taken))
        .chain(top.iter().cloned())
        .collect()
}

/// Whether the two entries name at least one thing in common.
pub fn same_thing(a: &Entry, b: &Entry) -> bool {
    let taken: HashSet<Key> = keys(a).into_iter().collect();
    keys(b).iter().any(|k| taken.contains(k))
}

/// One thing an entry puts in place for one agent.
pub type Claim = (Key, Agent);

/// What a resolved entry (agents filled in) puts in place: each of its keys for each agent.
pub fn claims(entry: &Entry) -> Vec<Claim> {
    let agents = entry.agents();
    keys(entry)
        .into_iter()
        .flat_map(|key| agents.iter().map(move |agent| (key.clone(), *agent)))
        .collect()
}

/// The part of a resolved entry that `have` does not cover: the names with a missing claim for
/// some agent, or the whole entry when it has one key. `None` when everything is there.
pub fn lacking(entry: &Entry, have: &HashSet<Claim>) -> Option<Entry> {
    let covered: HashSet<Key> = keys(entry)
        .into_iter()
        .filter(|key| {
            entry
                .agents()
                .iter()
                .all(|agent| have.contains(&(key.clone(), *agent)))
        })
        .collect();
    without(entry.clone(), &covered)
}

fn keys(entry: &Entry) -> Vec<Key> {
    match entry {
        Entry::Skill { source, names, .. } => per_name(source, names, Key::Skill),
        Entry::Subagent { source, names, .. } => per_name(source, names, Key::Subagent),
        Entry::Plugin {
            marketplace, name, ..
        } => vec![Key::Plugin(marketplace.clone(), name.clone())],
        Entry::Mcp { name, .. } => vec![Key::Mcp(name.clone())],
        Entry::Hook {
            event,
            matcher,
            command,
            ..
        } => vec![Key::Hook(event.clone(), matcher.clone(), command.clone())],
    }
}

fn per_name(source: &str, names: &[String], key: fn(String, String) -> Key) -> Vec<Key> {
    names
        .iter()
        .map(|name| key(source.to_string(), name.clone()))
        .collect()
}

/// The entry minus the names whose key is taken; `None` when nothing of it is left.
fn without(mut entry: Entry, taken: &HashSet<Key>) -> Option<Entry> {
    let all = keys(&entry);
    if !all.iter().any(|k| taken.contains(k)) {
        return Some(entry);
    }
    let (Entry::Skill { names, .. } | Entry::Subagent { names, .. }) = &mut entry else {
        return None;
    };
    let mut kept = all.iter().map(|k| !taken.contains(k));
    names.retain(|_| kept.next().unwrap_or(true));
    (!names.is_empty()).then_some(entry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Agent;

    fn skills(source: &str, names: &[&str]) -> Entry {
        Entry::Skill {
            source: source.into(),
            names: names.iter().map(|n| n.to_string()).collect(),
            agents: vec![],
        }
    }

    fn hook(matcher: Option<&str>, command: &str) -> Entry {
        Entry::Hook {
            event: "Stop".into(),
            matcher: matcher.map(String::from),
            command: command.into(),
            agents: vec![],
        }
    }

    fn mcp(name: &str, command: &str) -> Entry {
        Entry::Mcp {
            name: name.into(),
            command: Some(command.into()),
            args: vec![],
            url: None,
            transport: None,
            agents: vec![],
        }
    }

    #[test]
    fn only_the_names_missing_for_some_agent_are_lacking() {
        let entry = skills("o/r", &["a", "b"]).with_agents(vec![Agent::Claude, Agent::Codex]);
        let have: HashSet<Claim> = [
            (Key::Skill("o/r".into(), "a".into()), Agent::Claude),
            (Key::Skill("o/r".into(), "a".into()), Agent::Codex),
            (Key::Skill("o/r".into(), "b".into()), Agent::Claude),
        ]
        .into();

        let lacking = lacking(&entry, &have);

        // Assert b is still wanted for Codex and a is not.
        assert_eq!(
            lacking,
            Some(skills("o/r", &["b"]).with_agents(vec![Agent::Claude, Agent::Codex]))
        );
    }

    #[test]
    fn an_entry_whose_claims_are_all_there_lacks_nothing() {
        let entry = hook(None, "x").with_agents(vec![Agent::Claude]);
        let have = claims(&entry).into_iter().collect();

        assert_eq!(lacking(&entry, &have), None);
    }

    #[test]
    fn a_child_skill_replaces_the_parents_and_goes_last() {
        let base = vec![skills("o/r", &["tdd"]), skills("o/r", &["grill"])];
        let child = skills("o/r", &["tdd"]).with_agents(vec![Agent::Codex]);

        let merged = overlay(base, std::slice::from_ref(&child));

        // Assert the parent's tdd is gone and the child's comes after the survivor.
        assert_eq!(merged, [skills("o/r", &["grill"]), child]);
    }

    #[test]
    fn a_partly_overlapping_skill_entry_loses_only_the_shared_names() {
        let base = vec![skills("o/r", &["a", "b", "c"])];

        let merged = overlay(base, &[skills("o/r", &["b"])]);

        // Assert the others stay in the parent's entry.
        assert_eq!(merged, [skills("o/r", &["a", "c"]), skills("o/r", &["b"])]);
    }

    #[test]
    fn the_same_skill_name_from_another_source_is_a_different_skill() {
        let base = vec![skills("o/one", &["tdd"])];

        let merged = overlay(base, &[skills("o/two", &["tdd"])]);

        // Assert both remain.
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn an_mcp_server_is_replaced_by_name_whatever_its_launch() {
        let merged = overlay(vec![mcp("db", "old")], &[mcp("db", "new")]);

        // Assert only the child's definition remains.
        assert_eq!(merged, [mcp("db", "new")]);
    }

    #[test]
    fn a_hook_is_the_same_only_with_the_same_event_matcher_and_command() {
        let base = vec![hook(None, "x"), hook(Some("Bash"), "x")];

        let merged = overlay(base, &[hook(Some("Bash"), "x")]);

        // Assert only the matching handler is replaced.
        assert_eq!(merged, [hook(None, "x"), hook(Some("Bash"), "x")]);
    }

    #[test]
    fn entries_are_the_same_thing_when_they_share_a_key() {
        let a = skills("o/r", &["a", "b"]);

        assert!(same_thing(&a, &skills("o/r", &["b", "c"])));
        assert!(!same_thing(&a, &skills("o/r", &["c"])));
        assert!(!same_thing(&a, &mcp("a", "x")));
    }

    #[test]
    fn a_plugin_is_identified_by_marketplace_and_name() {
        let plugin = |source: &str| Entry::Plugin {
            source: source.into(),
            marketplace: "mp".into(),
            name: "p".into(),
            agents: vec![],
        };

        let merged = overlay(vec![plugin("o/old")], &[plugin("o/new")]);

        // Assert the source can change without keeping the parent's.
        assert_eq!(merged, [plugin("o/new")]);
    }
}
