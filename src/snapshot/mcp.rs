use std::collections::BTreeMap;

use super::{Accept, Collector, Reason, Skipped, merge_agents, own};
use crate::mcp::{Launch, Server};
use crate::model::{Agent, Augmentation, Kind};
use crate::profile::Entry;

/// One MCP server across agents: name, args (or the transport of a url server), command, url.
type ServerKey = (String, Vec<String>, Option<String>, Option<String>);

/// Servers that need no secrets, merged across agents.
#[derive(Default)]
pub(super) struct McpServers(BTreeMap<ServerKey, Vec<Agent>>);

impl Collector for McpServers {
    fn accept(&mut self, item: &Augmentation, agents: Vec<Agent>) -> Accept {
        if item.kind != Kind::Mcp {
            return Accept::Pass;
        }
        match Server::from_item(item) {
            Some(server) if server.secret_count() == 0 => {
                merge_agents(self.0.entry(key(server)).or_default(), agents);
                Accept::Taken
            }
            Some(server) => {
                Accept::Skipped(Skipped::new(item, Reason::Secrets(server.secret_count())))
            }
            None => Accept::Skipped(Skipped::new(item, Reason::Unreadable)),
        }
    }

    fn finish(&mut self, default: &[Agent]) -> Vec<Entry> {
        std::mem::take(&mut self.0)
            .into_iter()
            .map(|(key, agents)| entry(key, own(agents, default)))
            .collect()
    }
}

fn key(server: Server) -> ServerKey {
    match server.launch {
        Launch::Stdio { command, args } => (server.name, args, Some(command), None),
        Launch::Url { url, transport } => (server.name, vec![transport], None, Some(url)),
    }
}

fn entry((name, args, command, url): ServerKey, agents: Vec<Agent>) -> Entry {
    // A url server keeps its transport where a command keeps its args.
    let (args, transport) = match &url {
        Some(_) => (vec![], args.into_iter().next().filter(|t| t != "http")),
        None => (args, None),
    };
    Entry::Mcp {
        name,
        command,
        args,
        url,
        transport,
        agents,
    }
}
