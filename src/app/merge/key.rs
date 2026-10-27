use skillz_harness::mcp::Server;
use skillz_harness::model::{Details, Installed, Kind, Scope};

/// What makes two items the same thing seen through different agents. MCP servers are the same
/// when they launch the same way; the rest by kind, name, scope, group and source. A hook's name
/// is the program it runs, and each agent wires that program in with its own command line, so
/// hooks of one program are one item whatever their handlers say. A plugin's name already says its marketplace, and each
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
    Mcp(Server),
}

fn contents(item: &Installed) -> Contents {
    match (&item.kind, &item.details) {
        (Kind::Mcp, Details::Mcp { server, .. }) => Contents::Mcp(server.clone()),
        _ => Contents::None,
    }
}
