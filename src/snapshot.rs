//! Turn what is installed into a profile. Only what a profile can install again is kept: skills
//! `npx skills` fetched from a source, and plugins whose marketplace has one. Hand-made skills,
//! hooks and marketplaces (a plugin brings its own) are reported as skipped.

use std::collections::BTreeMap;

use crate::mcp::{Launch, Server};
use crate::model::{Agent, Augmentation, Installer, Kind};
use crate::profile::{Entry, Profile};

#[derive(Debug)]
pub struct Snapshot {
    pub profile: Profile,
    /// Labels of installed items the profile cannot reproduce.
    pub skipped: Vec<String>,
}

/// One MCP server across agents: name, args (or the transport of a url server), command, url.
type ServerKey = (String, Vec<String>, Option<String>, Option<String>);

pub fn snapshot(name: &str, items: &[Augmentation]) -> Snapshot {
    let mut skills: BTreeMap<(String, Vec<Agent>), Vec<String>> = BTreeMap::new();
    let mut plugins: BTreeMap<(String, String, String), Vec<Agent>> = BTreeMap::new();
    let mut servers: BTreeMap<ServerKey, Vec<Agent>> = BTreeMap::new();
    let mut subagents: BTreeMap<(String, Vec<Agent>), Vec<String>> = BTreeMap::new();
    let mut skipped = vec![];
    for item in items {
        let mut agents = item.agents.clone();
        agents.sort();
        match (item.kind, item.installer, &item.source) {
            (Kind::Skill, Installer::Npx, Some(source)) => {
                skills
                    .entry((source.clone(), agents))
                    .or_default()
                    .push(item.name.clone());
            }
            (Kind::Plugin, _, Some(source)) => {
                let plugin = item.name.split('@').next().unwrap_or(&item.name);
                let key = (source.clone(), item.group.clone(), plugin.to_string());
                let all = plugins.entry(key).or_default();
                all.extend(agents);
                all.sort();
                all.dedup();
            }
            (Kind::Subagent, _, Some(source)) => {
                // The file name is what a copy finds the definition by.
                let file = item.children.iter().find_map(|c| c.strip_prefix("file: "));
                let stem = file.and_then(|f| f.rsplit_once('.').map(|(s, _)| s));
                subagents
                    .entry((source.clone(), agents))
                    .or_default()
                    .push(stem.unwrap_or(&item.name).to_string());
            }
            (Kind::Mcp, ..) => match Server::from_item(item) {
                Some(server) if server.secrets.is_empty() => {
                    let key = match server.launch {
                        Launch::Stdio { command, args } => (server.name, args, Some(command), None),
                        Launch::Url { url, transport } => {
                            (server.name, vec![transport], None, Some(url))
                        }
                    };
                    let all = servers.entry(key).or_default();
                    all.extend(agents);
                    all.sort();
                    all.dedup();
                }
                Some(server) => skipped.push(format!(
                    "{} (has {} env vars or headers, which a profile does not carry)",
                    item.label(true),
                    server.secrets.len()
                )),
                None => skipped.push(item.label(true)),
            },
            (Kind::Marketplace, ..) => {}
            _ => skipped.push(item.label(true)),
        }
    }

    let mut profile = Profile::new(name);
    let own = |agents: Vec<Agent>| {
        if agents == profile.agents {
            vec![]
        } else {
            agents
        }
    };
    let mut entries = vec![];
    for ((source, agents), mut names) in skills {
        names.sort();
        names.dedup();
        entries.push(Entry::Skill {
            source,
            names,
            agents: own(agents),
        });
    }
    for ((source, marketplace, name), agents) in plugins {
        entries.push(Entry::Plugin {
            source,
            marketplace,
            name,
            agents: own(agents),
        });
    }
    for ((source, agents), mut names) in subagents {
        names.sort();
        names.dedup();
        entries.push(Entry::Subagent {
            source,
            names,
            agents: own(agents),
        });
    }
    for ((name, args, command, url), agents) in servers {
        // A url server keeps its transport where a command keeps its args.
        let (args, transport) = match &url {
            Some(_) => (vec![], args.into_iter().next().filter(|t| t != "http")),
            None => (args, None),
        };
        entries.push(Entry::Mcp {
            name,
            command,
            args,
            url,
            transport,
            agents: own(agents),
        });
    }
    for entry in entries {
        match entry.validate() {
            Ok(()) => profile.entries.push(entry),
            Err(_) => skipped.push(entry.to_string()),
        }
    }
    Snapshot { profile, skipped }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Scope;

    fn item(
        name: &str,
        kind: Kind,
        installer: Installer,
        group: &str,
        source: Option<&str>,
        agents: &[Agent],
    ) -> Augmentation {
        Augmentation {
            id: format!("{kind}:{name}:{agents:?}"),
            name: name.into(),
            kind,
            agents: agents.to_vec(),
            scope: Scope::Project,
            group: group.into(),
            source: source.map(Into::into),
            installer,
            children: vec![],
            removal: vec![],
        }
    }

    fn both() -> [Agent; 2] {
        [Agent::Claude, Agent::Codex]
    }

    #[test]
    fn npx_skills_group_by_source_and_agents() {
        let skill = |n: &str, src: &str, agents: &[Agent]| {
            item(n, Kind::Skill, Installer::Npx, src, Some(src), agents)
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
            item(
                "pp@mp",
                Kind::Plugin,
                Installer::ClaudeCli,
                "mp",
                Some("o/mp"),
                &[agent],
            )
        };
        let market = item(
            "mp",
            Kind::Marketplace,
            Installer::ClaudeCli,
            "mp",
            Some("o/mp"),
            &[Agent::Claude],
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
                item(
                    "mine",
                    Kind::Skill,
                    Installer::Local,
                    "local",
                    None,
                    &both(),
                ),
                item("hk", Kind::Hook, Installer::Local, "hooks", None, &both()),
                item(
                    "np@mp",
                    Kind::Plugin,
                    Installer::ClaudeCli,
                    "mp",
                    None,
                    &both(),
                ),
                item(
                    "-bad",
                    Kind::Skill,
                    Installer::Npx,
                    "o/r",
                    Some("o/r"),
                    &both(),
                ),
            ],
        );
        assert!(snap.profile.entries.is_empty());
        assert_eq!(snap.skipped.len(), 4);
    }

    #[test]
    fn mcp_servers_are_saved_unless_they_need_secrets() {
        let server = |name: &str, children: Vec<&str>, agent: Agent| {
            let mut i = item(name, Kind::Mcp, Installer::ClaudeCli, "mcp", None, &[agent]);
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
        assert_eq!(snap.skipped.len(), 2);
        assert!(
            snap.skipped
                .iter()
                .any(|s| s.contains("has 1 env") && !s.contains("TOKEN"))
        );
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
    fn subagents_with_a_source_are_saved_by_file_stem() {
        let agent = |name: &str, file: &str, source: Option<&str>| {
            let mut i = item(
                name,
                Kind::Subagent,
                Installer::Local,
                "g",
                source,
                &[Agent::Claude],
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
            &[item(
                "tdd",
                Kind::Skill,
                Installer::Npx,
                "o/r",
                Some("o/r"),
                &both(),
            )],
        );
        let text = snap.profile.to_text().unwrap();
        assert_eq!(Profile::parse("p", &text).unwrap(), snap.profile);
    }
}
