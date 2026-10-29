//! What makes two entries "the same thing", so a profile that extends another can replace its
//! entries instead of repeating them.
//!
//! A skill or subagent entry names several things, so it has one key per name. A child entry
//! replaces a parent entry holding the same key; a parent entry that only partly overlaps keeps
//! its other names, and disappears when none are left. The agents of an entry are not part of its
//! identity: the child's agents win along with the entry.
//!
//! A skill or subagent key holds a repository source as its identity, so `o/r` and
//! `https://github.com/o/r.git` name the same source. An `owner/repo` spelling always means the
//! repository; any other path stays as written (`source_identity`).

use std::collections::HashSet;

use skillz_harness::Spec;
use skillz_harness::clone::source_identity;
use skillz_harness::model::{Agent, Details, Installed, Installer, Kind, split_plugin_id};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    Skill(String, String),
    Subagent(String, String),
    Plugin(String, String),
    Mcp(String),
    Hook(String, Option<String>, String),
}

/// `base` without what `top` replaces, followed by `top`.
pub fn overlay(base: Vec<Spec>, top: &[Spec]) -> Vec<Spec> {
    let taken: HashSet<Key> = top.iter().flat_map(keys).collect();
    base.into_iter()
        .filter_map(|entry| without(entry, &taken))
        .chain(top.iter().cloned())
        .collect()
}

/// Whether the two entries name at least one thing in common.
pub fn same_thing(a: &Spec, b: &Spec) -> bool {
    let taken: HashSet<Key> = keys(a).into_iter().collect();
    keys(b).iter().any(|k| taken.contains(k))
}

/// One thing an entry puts in place for one agent.
pub type Claim = (Key, Agent);

/// What a resolved entry (agents filled in) puts in place: each of its keys for each agent.
pub fn claims(entry: &Spec) -> Vec<Claim> {
    let agents = entry.agents();
    keys(entry)
        .into_iter()
        .flat_map(|key| agents.iter().map(move |agent| (key.clone(), *agent)))
        .collect()
}

/// What is installed, independent of whether a profile can reproduce its credentials or source.
pub(crate) fn installed_claims(item: &Installed) -> Vec<Claim> {
    installed_keys(item)
        .into_iter()
        .flat_map(|key| item.agents.iter().map(move |agent| (key.clone(), *agent)))
        .collect()
}

/// A plugin also holds each skill it ships, from its marketplace's source, so a skill entry
/// installed through its owning plugin is in place.
pub(crate) fn installed_keys(item: &Installed) -> Vec<Key> {
    let source = || match item.installer {
        Installer::Npx => item.source.as_ref(),
        _ => item.source.as_ref().or(item.origin.link.as_ref()),
    };
    match item.kind {
        Kind::Skill => source()
            .map(|source| {
                let name = match &item.details {
                    Details::Skill { install_name, .. } => install_name,
                    _ => &item.name,
                };
                vec![sourced(Key::Skill, source, name)]
            })
            .unwrap_or_default(),
        Kind::Subagent => match (source(), &item.details) {
            (Some(source), Details::Subagent { file }) => file
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(|name| vec![sourced(Key::Subagent, source, name)])
                .unwrap_or_default(),
            _ => vec![],
        },
        Kind::Plugin => {
            let (name, marketplace) = split_plugin_id(&item.name);
            let shipped = source().into_iter().flat_map(|source| {
                item.plugin_skills()
                    .map(move |skill| sourced(Key::Skill, source, skill))
            });
            std::iter::once(Key::Plugin(marketplace.into(), name.into()))
                .chain(shipped)
                .collect()
        }
        Kind::Hook => item
            .hooks
            .iter()
            .map(|hook| {
                Key::Hook(
                    hook.event.clone(),
                    hook.matcher.clone(),
                    hook.command.clone(),
                )
            })
            .collect(),
        Kind::Mcp => vec![Key::Mcp(item.name.clone())],
        Kind::Marketplace => vec![],
    }
}

/// The part of a resolved entry that `have` does not cover: the names with a missing claim for
/// some agent, or the whole entry when it has one key. `None` when everything is there.
pub fn lacking(entry: &Spec, have: &HashSet<Claim>) -> Option<Spec> {
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

pub(crate) fn keys(entry: &Spec) -> Vec<Key> {
    match entry {
        Spec::Skill { source, names, .. } => per_name(source, names, Key::Skill),
        Spec::Subagent { source, names, .. } => per_name(source, names, Key::Subagent),
        Spec::Plugin {
            marketplace, name, ..
        } => vec![Key::Plugin(marketplace.clone(), name.clone())],
        Spec::Mcp { name, .. } => vec![Key::Mcp(name.clone())],
        Spec::Hook {
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
        .map(|name| sourced(key, source, name))
        .collect()
}

fn sourced(key: fn(String, String) -> Key, source: &str, name: &str) -> Key {
    key(source_identity(source), name.into())
}

/// The entry minus the names whose key is taken; `None` when nothing of it is left.
fn without(mut entry: Spec, taken: &HashSet<Key>) -> Option<Spec> {
    let all = keys(&entry);
    if !all.iter().any(|k| taken.contains(k)) {
        return Some(entry);
    }
    let (Spec::Skill { names, .. } | Spec::Subagent { names, .. }) = &mut entry else {
        return None;
    };
    let mut kept = all.iter().map(|k| !taken.contains(k));
    names.retain(|_| kept.next().unwrap_or(true));
    (!names.is_empty()).then_some(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skills(source: &str, names: &[&str]) -> Spec {
        Spec::Skill {
            source: source.into(),
            names: names.iter().map(|n| n.to_string()).collect(),
            agents: vec![],
        }
    }

    fn hook(matcher: Option<&str>, command: &str) -> Spec {
        Spec::Hook {
            event: "Stop".into(),
            matcher: matcher.map(String::from),
            command: command.into(),
            agents: vec![],
        }
    }

    fn mcp(name: &str, command: &str) -> Spec {
        Spec::Mcp {
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
        let both = skills("o/r", &["a"]).with_agents(vec![Agent::Claude, Agent::Codex]);
        let claude_only = skills("o/r", &["b"]).with_agents(vec![Agent::Claude]);
        let have: HashSet<Claim> = claims(&both)
            .into_iter()
            .chain(claims(&claude_only))
            .collect();

        let lacking = lacking(&entry, &have);

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

        assert_eq!(merged, [skills("o/r", &["grill"]), child]);
    }

    #[test]
    fn a_partly_overlapping_skill_entry_loses_only_the_shared_names() {
        let base = vec![skills("o/r", &["a", "b", "c"])];

        let merged = overlay(base, &[skills("o/r", &["b"])]);

        assert_eq!(merged, [skills("o/r", &["a", "c"]), skills("o/r", &["b"])]);
    }

    #[test]
    fn the_same_skill_name_from_another_source_is_a_different_skill() {
        let base = vec![skills("o/one", &["tdd"])];

        let merged = overlay(base, &[skills("o/two", &["tdd"])]);

        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn an_mcp_server_is_replaced_by_name_whatever_its_launch() {
        let merged = overlay(vec![mcp("db", "old")], &[mcp("db", "new")]);

        assert_eq!(merged, [mcp("db", "new")]);
    }

    #[test]
    fn a_hook_is_the_same_only_with_the_same_event_matcher_and_command() {
        let base = vec![hook(None, "x"), hook(Some("Bash"), "x")];

        let merged = overlay(base, &[hook(Some("Bash"), "x")]);

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
    fn a_source_is_the_same_however_it_is_spelled() {
        let short = skills("o/r", &["a"]);

        assert!(same_thing(
            &short,
            &skills("https://github.com/o/r.git", &["a"])
        ));
        assert!(same_thing(
            &short,
            &skills("git@github.com:O/R.git", &["a"])
        ));
        assert!(!same_thing(&short, &skills("o/other", &["a"])));
    }

    #[test]
    fn local_paths_differing_only_by_case_are_different_sources() {
        let lower = skills("/src/skills", &["a"]);

        assert!(!same_thing(&lower, &skills("/src/Skills", &["a"])));
        assert!(same_thing(&lower, &skills("/src/skills", &["a"])));
    }

    fn installed(name: &str, kind: Kind, source: Option<&str>) -> Installed {
        Installed {
            details: Default::default(),
            hooks: vec![],
            members: vec![],
            id: format!("{kind}:{name}"),
            name: name.into(),
            kind,
            agents: vec![Agent::Claude],
            scope: skillz_harness::model::Scope::Project,
            group: "g".into(),
            source: source.map(Into::into),
            installer: Installer::Npx,
            origin: Default::default(),
            children: vec![],
            removal: vec![],
        }
    }

    fn plugin_shipping(source: Option<&str>, shipped: &[&str]) -> Installed {
        Installed {
            details: Details::Plugin {
                location: "/plugins/p".into(),
                skills: shipped.iter().map(|s| s.to_string()).collect(),
            },
            installer: Installer::ClaudeCli,
            ..installed("p@mp", Kind::Plugin, source)
        }
    }

    #[test]
    fn a_plugin_holds_itself_and_each_skill_it_ships_from_its_source() {
        let plugin = plugin_shipping(Some("o/r"), &["a", "b"]);

        let held = installed_keys(&plugin);

        let mut expected = vec![Key::Plugin("mp".into(), "p".into())];
        expected.extend(keys(&skills("https://github.com/o/r.git", &["a", "b"])));
        assert_eq!(held, expected);
    }

    #[test]
    fn a_plugin_without_a_known_source_holds_only_itself() {
        let plugin = plugin_shipping(None, &["a"]);

        assert_eq!(
            installed_keys(&plugin),
            [Key::Plugin("mp".into(), "p".into())]
        );
    }

    #[test]
    fn a_plugin_is_identified_by_marketplace_and_name() {
        let plugin = |source: &str| Spec::Plugin {
            source: source.into(),
            marketplace: "mp".into(),
            name: "p".into(),
            agents: vec![],
        };

        let merged = overlay(vec![plugin("o/old")], &[plugin("o/new")]);

        assert_eq!(merged, [plugin("o/new")]);
    }
}
