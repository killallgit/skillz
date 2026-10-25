use crate::harness::mcp::Server;
use crate::harness::model::{Details, Installed, Kind, Scope};

/// What makes two items the same thing seen through different agents. Hooks are the same when
/// they hold the same handlers and MCP servers when they launch the same way; the rest by
/// kind, name, scope, group and source. A plugin's name already says its marketplace, and each
/// agent records the marketplace's source its own way (a URL, a path, nothing), so the source
/// is no part of its identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct Key {
    kind: Kind,
    scope: Scope,
    name: String,
    group: String,
    source: Option<String>,
    contents: Contents,
}

impl Key {
    pub(super) fn of(item: &Installed) -> Self {
        Key {
            kind: item.kind,
            scope: item.scope,
            name: item.name.clone(),
            group: item.group.clone(),
            source: (item.kind != Kind::Plugin)
                .then(|| item.source.clone())
                .flatten(),
            contents: contents(item),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Contents {
    None,
    Hooks(Vec<(String, Option<String>, String)>),
    Mcp(Server),
}

fn contents(item: &Installed) -> Contents {
    match (&item.kind, &item.details) {
        (Kind::Hook, _) => {
            let mut hooks: Vec<_> = item
                .hooks
                .iter()
                .map(|h| (h.event.clone(), h.matcher.clone(), h.command.clone()))
                .collect();
            hooks.sort();
            hooks.dedup();
            Contents::Hooks(hooks)
        }
        (Kind::Mcp, Details::Mcp { server, .. }) => Contents::Mcp(server.clone()),
        _ => Contents::None,
    }
}
