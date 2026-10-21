//! Turn what is installed into a profile. Only what a profile can install again is kept: skills
//! `npx skills` fetched from a source, plugins whose marketplace has one, subagents with a
//! source, hooks, and MCP servers that need no secrets. Everything else is reported as skipped.
//!
//! Each kind of entry has its own `Collector`; `snapshot` only routes items to them.

mod hooks;
mod mcp;
mod plugins;
mod skills;
mod subagents;

use std::fmt;

use crate::model::{Agent, Augmentation};
use crate::profile::{Entry, Profile};

#[derive(Debug)]
pub struct Snapshot {
    pub profile: Profile,
    /// Installed items the profile cannot reproduce.
    pub skipped: Vec<Skipped>,
}

/// An installed item left out of a snapshot, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub label: String,
    pub reason: Reason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// Nothing to reinstall it from: a hand-made skill, a plugin without a marketplace.
    NoSource,
    /// An MCP server with this many env vars or headers, which a profile does not carry.
    Secrets(usize),
    /// The item's definition could not be read back into an entry.
    Unreadable,
    /// The entry built from it is not a valid profile entry.
    Invalid,
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Reason::NoSource => write!(f, "nothing to reinstall it from"),
            Reason::Secrets(n) => write!(f, "has {n} env vars or headers a profile does not carry"),
            Reason::Unreadable => write!(f, "its definition cannot be read back"),
            Reason::Invalid => write!(f, "not a valid profile entry"),
        }
    }
}

impl Skipped {
    fn new(item: &Augmentation, reason: Reason) -> Self {
        Skipped {
            label: item.label(true),
            reason,
        }
    }
}

/// What a collector made of one item.
enum Accept {
    Taken,
    Skipped(Skipped),
    /// Not this collector's kind of item.
    Pass,
}

trait Collector {
    fn accept(&mut self, item: &Augmentation, agents: Vec<Agent>) -> Accept;
    /// The entries gathered so far; an entry for exactly the profile's agents names none.
    fn finish(&mut self, default: &[Agent]) -> Vec<Entry>;
}

fn collectors() -> Vec<Box<dyn Collector>> {
    vec![
        Box::<skills::Skills>::default(),
        Box::<plugins::Plugins>::default(),
        Box::<subagents::Subagents>::default(),
        Box::<mcp::McpServers>::default(),
        Box::<hooks::Hooks>::default(),
    ]
}

pub fn snapshot(name: &str, items: &[Augmentation]) -> Snapshot {
    let mut collectors = collectors();
    let mut skipped = vec![];
    for item in items {
        if let Some(left_out) = route(&mut collectors, item) {
            skipped.push(left_out);
        }
    }
    let mut profile = Profile::new(name);
    let default = profile.agents.clone();
    for collector in &mut collectors {
        for entry in collector.finish(&default) {
            match entry.validate() {
                Ok(()) => profile.entries.push(entry),
                Err(_) => skipped.push(Skipped {
                    label: entry.to_string(),
                    reason: Reason::Invalid,
                }),
            }
        }
    }
    Snapshot { profile, skipped }
}

/// Hand `item` to the first collector that wants it; `Some` when none can keep it.
fn route(collectors: &mut [Box<dyn Collector>], item: &Augmentation) -> Option<Skipped> {
    let mut agents = item.agents.clone();
    agents.sort();
    for collector in collectors {
        match collector.accept(item, agents.clone()) {
            Accept::Taken => return None,
            Accept::Skipped(skipped) => return Some(skipped),
            Accept::Pass => {}
        }
    }
    Some(Skipped::new(item, Reason::NoSource))
}

fn own(agents: Vec<Agent>, default: &[Agent]) -> Vec<Agent> {
    if agents == default { vec![] } else { agents }
}

fn merge_agents(all: &mut Vec<Agent>, more: Vec<Agent>) {
    all.extend(more);
    all.sort();
    all.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Installer, Kind, Scope};

    fn item(name: &str, kind: Kind, installer: Installer, agents: &[Agent]) -> Augmentation {
        Augmentation {
            hooks: Vec::new(),
            id: format!("{kind}:{name}:{agents:?}"),
            name: name.into(),
            kind,
            agents: agents.to_vec(),
            scope: Scope::Project,
            group: "local".into(),
            source: None,
            installer,
            origin: Default::default(),
            children: vec![],
            removal: vec![],
        }
    }

    fn in_group(item: Augmentation, group: &str, source: Option<&str>) -> Augmentation {
        Augmentation {
            group: group.into(),
            source: source.map(Into::into),
            ..item
        }
    }

    fn both() -> [Agent; 2] {
        [Agent::Claude, Agent::Codex]
    }

    #[test]
    fn npx_skills_group_by_source_and_agents() {
        let skill = |n: &str, src: &str, agents: &[Agent]| {
            in_group(item(n, Kind::Skill, Installer::Npx, agents), src, Some(src))
        };
        let snap = snapshot(
            "p",
            &[
                skill("tdd", "o/r", &both()),
                skill("grill", "o/r", &both()),
                skill("only", "o/r", &[Agent::Claude]),
                skill("x", "a/b", &both()),
            ],
        );
        assert!(snap.skipped.is_empty());
        let names: Vec<(&str, Vec<&str>, usize)> = snap
            .profile
            .entries
            .iter()
            .map(|e| match e {
                Entry::Skill {
                    source,
                    names,
                    agents,
                } => (
                    source.as_str(),
                    names.iter().map(String::as_str).collect(),
                    agents.len(),
                ),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(
            names,
            [
                ("a/b", vec!["x"], 0),
                ("o/r", vec!["only"], 1),
                ("o/r", vec!["grill", "tdd"], 0),
            ]
        );
    }

    #[test]
    fn a_plugin_installed_for_two_agents_is_one_entry() {
        let plugin = |agent: Agent| {
            in_group(
                item("pp@mp", Kind::Plugin, Installer::ClaudeCli, &[agent]),
                "mp",
                Some("o/mp"),
            )
        };
        let market = in_group(
            item(
                "mp",
                Kind::Marketplace,
                Installer::ClaudeCli,
                &[Agent::Claude],
            ),
            "mp",
            Some("o/mp"),
        );
        let snap = snapshot("p", &[plugin(Agent::Claude), plugin(Agent::Codex), market]);
        assert!(snap.skipped.is_empty());
        assert_eq!(
            snap.profile.entries,
            [Entry::Plugin {
                source: "o/mp".into(),
                marketplace: "mp".into(),
                name: "pp".into(),
                agents: vec![],
            }]
        );
    }

    #[test]
    fn what_cannot_be_reproduced_is_skipped_not_dropped_silently() {
        let snap = snapshot(
            "p",
            &[
                in_group(
                    item("mine", Kind::Skill, Installer::Local, &both()),
                    "local",
                    None,
                ),
                in_group(
                    item("hk", Kind::Hook, Installer::Local, &both()),
                    "hooks",
                    None,
                ),
                in_group(
                    item("np@mp", Kind::Plugin, Installer::ClaudeCli, &both()),
                    "mp",
                    None,
                ),
                in_group(
                    item("-bad", Kind::Skill, Installer::Npx, &both()),
                    "o/r",
                    Some("o/r"),
                ),
            ],
        );
        assert!(snap.profile.entries.is_empty());
        let reasons: Vec<Reason> = snap.skipped.iter().map(|s| s.reason).collect();
        // Assert each left-out item carries why it was left out.
        assert_eq!(
            reasons,
            [
                Reason::NoSource,
                Reason::Unreadable,
                Reason::NoSource,
                Reason::Invalid
            ]
        );
    }

    #[test]
    fn hooks_are_read_from_typed_data_and_merged_across_agents() {
        let handler =
            |event: &str, matcher: Option<&str>, command: &str| crate::hook_text::HookEntry {
                event: event.into(),
                matcher: matcher.map(String::from),
                command: command.into(),
            };
        let hook = |agent: Agent| {
            let mut i = in_group(
                item("fmt", Kind::Hook, Installer::Local, &[agent]),
                "hooks",
                None,
            );
            i.hooks = vec![
                handler("PostToolUse", Some("Edit"), "cargo fmt"),
                handler("Stop", None, "notify: done"),
            ];
            i
        };
        let snap = snapshot("p", &[hook(Agent::Claude), hook(Agent::Codex)]);
        // Assert both handlers become hook entries owned by both agents.
        assert_eq!(
            snap.profile.entries,
            [
                Entry::Hook {
                    event: "PostToolUse".into(),
                    matcher: Some("Edit".into()),
                    command: "cargo fmt".into(),
                    agents: vec![],
                },
                Entry::Hook {
                    event: "Stop".into(),
                    matcher: None,
                    command: "notify: done".into(),
                    agents: vec![],
                },
            ]
        );
        assert!(snap.skipped.is_empty());
    }

    #[test]
    fn a_matcher_containing_a_colon_survives_the_snapshot() {
        let mut i = in_group(
            item("fmt", Kind::Hook, Installer::Local, &both()),
            "hooks",
            None,
        );
        i.hooks = vec![crate::hook_text::HookEntry {
            event: "PreToolUse".into(),
            matcher: Some("mcp__a: b".into()),
            command: "run".into(),
        }];

        let snap = snapshot("p", &[i]);

        // Assert the matcher is kept whole.
        assert!(matches!(
            &snap.profile.entries[0],
            Entry::Hook { matcher: Some(m), .. } if m == "mcp__a: b"
        ));
    }

    #[test]
    fn mcp_servers_are_saved_unless_they_need_secrets() {
        let server = |name: &str, children: Vec<&str>, agent: Agent| {
            let mut i = in_group(
                item(name, Kind::Mcp, Installer::ClaudeCli, &[agent]),
                "mcp",
                None,
            );
            i.children = children.into_iter().map(String::from).collect();
            i
        };
        let snap = snapshot(
            "p",
            &[
                server(
                    "fs",
                    vec!["command: npx", "arg: -y", "arg: fs"],
                    Agent::Claude,
                ),
                server(
                    "fs",
                    vec!["command: npx", "arg: -y", "arg: fs"],
                    Agent::Codex,
                ),
                server(
                    "web",
                    vec!["url: https://x/mcp", "transport: sse"],
                    Agent::Claude,
                ),
                server(
                    "plain",
                    vec!["url: https://y/mcp", "transport: http"],
                    Agent::Claude,
                ),
                server("keyed", vec!["command: srv", "needs: TOKEN"], Agent::Claude),
                server("broken", vec![], Agent::Claude),
            ],
        );
        let reasons: Vec<Reason> = snap.skipped.iter().map(|s| s.reason).collect();
        // Assert the secret-bearing and the unreadable server are skipped for their own reasons.
        assert_eq!(reasons, [Reason::Secrets(1), Reason::Unreadable]);
        let get = |n: &str| {
            snap.profile.entries.iter().find_map(|e| match e {
                Entry::Mcp { name, .. } if name == n => Some(e.clone()),
                _ => None,
            })
        };
        assert_eq!(
            get("fs"),
            Some(Entry::Mcp {
                name: "fs".into(),
                command: Some("npx".into()),
                args: vec!["-y".into(), "fs".into()],
                url: None,
                transport: None,
                agents: vec![],
            })
        );
        let Some(Entry::Mcp { transport, .. }) = get("web") else {
            panic!()
        };
        assert_eq!(transport.as_deref(), Some("sse"));
        let Some(Entry::Mcp { transport, .. }) = get("plain") else {
            panic!()
        };
        assert_eq!(transport, None);
    }

    #[test]
    fn mcp_servers_with_credentials_in_args_or_url_are_not_saved() {
        let server = |name: &str, children: Vec<&str>| {
            let mut i = in_group(
                item(name, Kind::Mcp, Installer::ClaudeCli, &[Agent::Claude]),
                "mcp",
                None,
            );
            i.children = children.into_iter().map(String::from).collect();
            i
        };
        let snap = snapshot(
            "p",
            &[
                server("flag", vec!["command: srv", "arg: --api-key", "arg: abc"]),
                server("eq", vec!["command: srv", "arg: --token=abc"]),
                server(
                    "query",
                    vec!["url: https://x/mcp?key=abc", "transport: http"],
                ),
                server("user", vec!["url: https://u:p@x/mcp", "transport: http"]),
                server("clean", vec!["command: srv", "arg: --port", "arg: 80"]),
            ],
        );

        let reasons: Vec<Reason> = snap.skipped.iter().map(|s| s.reason).collect();

        // every credential carrier is skipped as Secrets; only the clean server is saved
        assert_eq!(reasons, [Reason::Secrets(1); 4]);
        assert_eq!(snap.profile.entries.len(), 1);
    }

    #[test]
    fn subagents_with_a_source_are_saved_by_file_stem() {
        let agent = |name: &str, file: &str, source: Option<&str>| {
            let mut i = in_group(
                item(name, Kind::Subagent, Installer::Local, &[Agent::Claude]),
                "g",
                source,
            );
            i.children = vec![format!("file: {file}")];
            i
        };
        let snap = snapshot(
            "p",
            &[
                agent("Frontend Developer", "eng-frontend.md", Some("o/agents")),
                agent("UI Designer", "ui.md", Some("o/agents")),
                agent("Mine", "mine.md", None),
            ],
        );
        // Assert only the source-less subagent is skipped.
        assert_eq!(snap.skipped[0].reason, Reason::NoSource);
        assert_eq!(snap.skipped.len(), 1);
        assert_eq!(
            snap.profile.entries,
            [Entry::Subagent {
                source: "o/agents".into(),
                names: vec!["eng-frontend".into(), "ui".into()],
                agents: vec![Agent::Claude],
            }]
        );
    }

    #[test]
    fn the_snapshot_is_a_valid_profile_that_round_trips() {
        let snap = snapshot(
            "p",
            &[in_group(
                item("tdd", Kind::Skill, Installer::Npx, &both()),
                "o/r",
                Some("o/r"),
            )],
        );
        let text = snap.profile.to_text().unwrap();
        assert_eq!(Profile::parse("p", &text).unwrap(), snap.profile);
    }
}
