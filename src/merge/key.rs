use crate::mcp::Server;
use crate::model::{Augmentation, Details, Kind, Scope};

/// What makes two items the same thing seen through different agents. Hooks are the same when
/// they hold the same handlers and MCP servers when they launch the same way; the rest by
/// kind, name, scope, group and source.
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
    pub(super) fn of(item: &Augmentation) -> Self {
        Key {
            kind: item.kind,
            scope: item.scope,
            name: item.name.clone(),
            group: item.group.clone(),
            source: item.source.clone(),
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

fn contents(item: &Augmentation) -> Contents {
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
