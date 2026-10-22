use crate::hook_text::HookEntry;
use crate::model::{Augmentation, Kind, Scope};

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
    contents: Vec<String>,
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

fn contents(item: &Augmentation) -> Vec<String> {
    let mut contents: Vec<String> = match item.kind {
        Kind::Hook => item.hooks.iter().map(HookEntry::label).collect(),
        Kind::Mcp => item.children.clone(),
        _ => return Vec::new(),
    };
    contents.sort();
    contents.dedup();
    contents
}
