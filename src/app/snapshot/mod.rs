//! Reinstallable entries from installed facts, grouped into a profile.

mod extract;

use std::collections::BTreeMap;
use std::fmt;

use crate::app::profile::Profile;
use skillz_harness::Spec;
use skillz_harness::mcp::Launch;
use skillz_harness::model::{Agent, Installed};

pub use extract::entries;

#[derive(Debug)]
pub struct Snapshot {
    pub profile: Profile,
    pub skipped: Vec<Skipped>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub label: String,
    pub reason: Reason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    NoSource,
    Secrets(usize),
    Unreadable,
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
    pub(crate) fn new(item: &Installed, reason: Reason) -> Self {
        Self {
            label: item.label(true),
            reason,
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ServerKey {
    name: String,
    launch: Launch,
}

#[derive(Default)]
struct Gathered {
    skills: BTreeMap<(String, String), Vec<Agent>>,
    plugins: BTreeMap<(String, String, String), Vec<Agent>>,
    subagents: BTreeMap<(String, String), Vec<Agent>>,
    servers: BTreeMap<ServerKey, Vec<Agent>>,
    hooks: BTreeMap<(String, Option<String>, String), Vec<Agent>>,
}

impl Gathered {
    fn add(&mut self, entry: Spec) {
        match entry {
            Spec::Skill {
                source,
                names,
                agents,
            } => add_named(&mut self.skills, source, names, agents),
            Spec::Subagent {
                source,
                names,
                agents,
            } => add_named(&mut self.subagents, source, names, agents),
            Spec::Plugin {
                source,
                marketplace,
                name,
                agents,
            } => self
                .plugins
                .entry((source, marketplace, name))
                .or_default()
                .extend(agents),
            Spec::Hook {
                event,
                matcher,
                command,
                agents,
            } => self
                .hooks
                .entry((event, matcher, command))
                .or_default()
                .extend(agents),
            Spec::Mcp {
                name,
                command,
                args,
                url,
                transport,
                agents,
            } => {
                let launch = match (command, url) {
                    (Some(command), _) => Launch::Stdio { command, args },
                    (_, Some(url)) => Launch::Url {
                        url,
                        transport: transport.unwrap_or_else(|| "http".into()),
                    },
                    _ => unreachable!("entries validates MCP launches"),
                };
                self.servers
                    .entry(ServerKey { name, launch })
                    .or_default()
                    .extend(agents);
            }
        }
    }

    fn finish(self) -> Vec<Spec> {
        let mut entries = Vec::new();
        entries.extend(
            group_named(self.skills).map(|((source, agents), names)| Spec::Skill {
                source,
                names,
                agents,
            }),
        );
        entries.extend(
            self.plugins
                .into_iter()
                .map(|((source, marketplace, name), agents)| Spec::Plugin {
                    source,
                    marketplace,
                    name,
                    agents,
                }),
        );
        entries.extend(
            group_named(self.subagents).map(|((source, agents), names)| Spec::Subagent {
                source,
                names,
                agents,
            }),
        );
        entries.extend(
            self.servers
                .into_iter()
                .map(|(key, agents)| extract::server_entry(key.name, key.launch, agents)),
        );
        entries.extend(
            self.hooks
                .into_iter()
                .map(|((event, matcher, command), agents)| Spec::Hook {
                    event,
                    matcher,
                    command,
                    agents,
                }),
        );
        entries
            .into_iter()
            .map(|entry| {
                let mut agents = entry.agents().to_vec();
                agents.sort();
                agents.dedup();
                entry.with_agents(agents)
            })
            .collect()
    }
}

fn add_named(
    entries: &mut BTreeMap<(String, String), Vec<Agent>>,
    source: String,
    names: Vec<String>,
    agents: Vec<Agent>,
) {
    for name in names {
        entries
            .entry((source.clone(), name))
            .or_default()
            .extend(agents.iter().copied());
    }
}

fn group_named(
    entries: BTreeMap<(String, String), Vec<Agent>>,
) -> impl Iterator<Item = ((String, Vec<Agent>), Vec<String>)> {
    let mut groups: BTreeMap<(String, Vec<Agent>), Vec<String>> = BTreeMap::new();
    for ((source, name), mut agents) in entries {
        agents.sort();
        agents.dedup();
        groups.entry((source, agents)).or_default().push(name);
    }
    groups.into_iter()
}

/// Reinstallable entries with exact agents, without constructing a profile.
pub fn collect(items: &[Installed]) -> (Vec<Spec>, Vec<Skipped>) {
    let mut gathered = Gathered::default();
    let mut skipped = Vec::new();
    for item in items.iter().flat_map(Installed::parts) {
        match entries(item) {
            Ok(entries) => entries.into_iter().for_each(|entry| gathered.add(entry)),
            Err(reason) => skipped.push(Skipped::new(item, reason)),
        }
    }
    (gathered.finish(), skipped)
}

pub fn snapshot(name: &str, items: &[Installed]) -> Snapshot {
    let (entries, skipped) = collect(items);
    let mut profile = Profile::new(name);
    profile.entries = entries
        .into_iter()
        .map(|entry| {
            let agents = entry.agents().to_vec();
            entry.with_agents(if agents == profile.agents {
                vec![]
            } else {
                agents
            })
        })
        .collect();
    Snapshot { profile, skipped }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skillz_harness::mcp::Server;
    use skillz_harness::model::{Details, Installer, Kind, Origin, Scope};

    pub(super) fn item(
        name: &str,
        kind: Kind,
        installer: Installer,
        agents: &[Agent],
    ) -> Installed {
        Installed {
            details: Default::default(),
            hooks: Vec::new(),
            members: Vec::new(),
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

    fn in_group(item: Installed, group: &str, source: Option<&str>) -> Installed {
        Installed {
            group: group.into(),
            source: source.map(Into::into),
            ..item
        }
    }

    pub(super) fn both() -> [Agent; 2] {
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
                Spec::Skill {
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
            [Spec::Plugin {
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
        let handler = |event: &str, matcher: Option<&str>, command: &str| {
            skillz_harness::hook_text::HookEntry {
                event: event.into(),
                matcher: matcher.map(String::from),
                command: command.into(),
            }
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

        assert_eq!(
            snap.profile.entries,
            [
                Spec::Hook {
                    event: "PostToolUse".into(),
                    matcher: Some("Edit".into()),
                    command: "cargo fmt".into(),
                    agents: vec![],
                },
                Spec::Hook {
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
        i.hooks = vec![skillz_harness::hook_text::HookEntry {
            event: "PreToolUse".into(),
            matcher: Some("mcp__a: b".into()),
            command: "run".into(),
        }];

        let snap = snapshot("p", &[i]);

        assert!(matches!(
            &snap.profile.entries[0],
            Spec::Hook { matcher: Some(m), .. } if m == "mcp__a: b"
        ));
    }

    fn server(name: &str, launch: Launch, secrets: &[&str], agent: Agent) -> Installed {
        Installed {
            details: Details::Mcp {
                config: "/config".into(),
                server: Server {
                    name: name.into(),
                    launch,
                    secrets: secrets.iter().map(|s| s.to_string()).collect(),
                },
            },
            ..item(name, Kind::Mcp, Installer::ClaudeCli, &[agent])
        }
    }

    fn stdio(command: &str, args: &[&str]) -> Launch {
        Launch::Stdio {
            command: command.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn url(url: &str, transport: &str) -> Launch {
        Launch::Url {
            url: url.into(),
            transport: transport.into(),
        }
    }

    #[test]
    fn mcp_servers_are_saved_unless_they_need_secrets() {
        let snap = snapshot(
            "p",
            &[
                server("fs", stdio("npx", &["-y", "fs"]), &[], Agent::Claude),
                server("fs", stdio("npx", &["-y", "fs"]), &[], Agent::Codex),
                server("web", url("https://x/mcp", "sse"), &[], Agent::Claude),
                server("plain", url("https://y/mcp", "http"), &[], Agent::Claude),
                server("keyed", stdio("srv", &[]), &["TOKEN"], Agent::Claude),
                item("broken", Kind::Mcp, Installer::ClaudeCli, &[Agent::Claude]),
            ],
        );
        assert_eq!(
            snap.skipped.iter().map(|s| s.reason).collect::<Vec<_>>(),
            [Reason::Secrets(1), Reason::Unreadable]
        );
        let get = |n: &str| {
            snap.profile
                .entries
                .iter()
                .find(|e| matches!(e, Spec::Mcp { name, .. } if name == n))
        };
        assert_eq!(
            get("fs"),
            Some(&Spec::Mcp {
                name: "fs".into(),
                command: Some("npx".into()),
                args: vec!["-y".into(), "fs".into()],
                url: None,
                transport: None,
                agents: vec![]
            })
        );
        assert!(
            matches!(get("web"), Some(Spec::Mcp { transport: Some(transport), .. }) if transport == "sse")
        );
        assert!(matches!(
            get("plain"),
            Some(Spec::Mcp {
                transport: None,
                ..
            })
        ));
    }

    #[test]
    fn mcp_servers_with_credentials_in_args_or_url_are_not_saved() {
        let snap = snapshot(
            "p",
            &[
                server(
                    "flag",
                    stdio("srv", &["--api-key", "abc"]),
                    &[],
                    Agent::Claude,
                ),
                server("eq", stdio("srv", &["--token=abc"]), &[], Agent::Claude),
                server(
                    "query",
                    url("https://x/mcp?key=abc", "http"),
                    &[],
                    Agent::Claude,
                ),
                server("user", url("https://u:p@x/mcp", "http"), &[], Agent::Claude),
                server("clean", stdio("srv", &["--port", "80"]), &[], Agent::Claude),
            ],
        );
        assert_eq!(
            snap.skipped.iter().map(|s| s.reason).collect::<Vec<_>>(),
            [Reason::Secrets(1); 4]
        );
        assert_eq!(snap.profile.entries.len(), 1);
    }

    #[test]
    fn handmade_skills_keep_their_install_name_without_reading_display_lines() {
        let skill = Installed {
            details: Details::Skill {
                directory: "/p/skills/folder".into(),
                install_name: "real".into(),
            },
            origin: Origin {
                link: Some("/p/skills".into()),
                version: None,
            },
            children: vec!["arbitrary presentation".into()],
            ..item("folder", Kind::Skill, Installer::Local, &both())
        };
        let snap = snapshot("p", &[skill]);
        assert_eq!(
            snap.profile.entries,
            [Spec::Skill {
                source: "/p/skills".into(),
                names: vec!["real".into()],
                agents: vec![]
            }]
        );
    }

    #[test]
    fn a_handmade_subagent_uses_its_directory_as_the_reinstall_source() {
        let definition = Installed {
            details: Details::Subagent {
                file: "/p/agents/reviewer.md".into(),
            },
            origin: Origin {
                link: Some("/p/agents".into()),
                version: None,
            },
            ..item(
                "Reviewer",
                Kind::Subagent,
                Installer::Local,
                &[Agent::Claude],
            )
        };
        let snap = snapshot("p", &[definition]);
        assert_eq!(
            snap.profile.entries,
            [Spec::Subagent {
                source: "/p/agents".into(),
                names: vec!["reviewer".into()],
                agents: vec![Agent::Claude]
            }]
        );
    }

    #[test]
    fn subagents_with_a_source_are_saved_by_file_stem() {
        let agent = |name: &str, file: &str, source: Option<&str>| {
            let mut i = in_group(
                item(name, Kind::Subagent, Installer::Local, &[Agent::Claude]),
                "g",
                source,
            );
            i.details = Details::Subagent { file: file.into() };
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

        assert_eq!(snap.skipped[0].reason, Reason::NoSource);
        assert_eq!(snap.skipped.len(), 1);
        assert_eq!(
            snap.profile.entries,
            [Spec::Subagent {
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
    #[test]
    fn merged_copies_keep_every_agents_installed_claim() {
        let skill = |agent| {
            in_group(
                item("tdd", Kind::Skill, Installer::Npx, &[agent]),
                "o/r",
                Some("o/r"),
            )
        };
        let merged = crate::app::merge::merge(vec![skill(Agent::Claude), skill(Agent::Codex)]);
        let snap = snapshot("p", &merged);
        assert_eq!(
            snap.profile.resolved(),
            [Spec::Skill {
                source: "o/r".into(),
                names: vec!["tdd".into()],
                agents: both().to_vec()
            }]
        );
    }
    #[test]
    fn source_credentials_are_skipped_without_echoing_them() {
        let skill = in_group(
            item("reviewer", Kind::Skill, Installer::Npx, &both()),
            "source",
            Some("https://user:snapshot-secret@host/repo.git"),
        );
        let result = snapshot("safe", &[skill]);
        assert!(result.profile.entries.is_empty());
        assert_eq!(result.skipped.len(), 1);
        assert!(!result.skipped[0].label.contains("snapshot-secret"));
    }
}
