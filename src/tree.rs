use crate::merge::tag;
use crate::model::{Augmentation, Kind, Scope};
use crate::package::{Package, group_by};

mod table;

pub use table::{Widths, render};

/// Table columns after the name, shared by the terminal list and the plain output.
pub const COLUMNS: [&str; 4] = ["KIND", "AGENTS", "SCOPE", "ITEMS"];

/// A display tree node.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Node {
    /// The row's name; group rows read like `source` or `claude hooks`.
    pub label: String,
    /// What the info modal shows for this row; empty for detail leaves, which have none.
    pub info: Vec<String>,
    /// The table columns after the name, as `COLUMNS` lists them.
    pub cells: [String; COLUMNS.len()],
    pub children: Vec<Node>,
    /// Index into `build`'s input for item rows; `None` for group headers and detail leaves.
    pub item: Option<usize>,
    /// Scope of a group header; `None` below the top level.
    pub scope: Option<Scope>,
}

impl Node {
    /// Leaves are display-only detail (often long shell commands), so cap their width.
    fn leaf(label: impl Into<String>) -> Self {
        const MAX: usize = 100;
        let label: String = label.into();
        let label = if label.chars().count() > MAX {
            label.chars().take(MAX - 1).chain(['…']).collect()
        } else {
            label
        };
        Node {
            label,
            ..Node::default()
        }
    }
}

/// Group augmentations for display: scope -> package -> item -> item.children. Project groups
/// come first, then global; within a scope see `package::group_by`. Deterministic.
pub fn build(items: &[Augmentation]) -> Vec<Node> {
    [Scope::Project, Scope::Global]
        .into_iter()
        .flat_map(|scope| {
            let here: Vec<Member> = items
                .iter()
                .enumerate()
                .filter(|(_, item)| item.scope == scope)
                .collect();
            group_by(here, |(_, item)| Package::of(item))
        })
        .map(|(package, members)| group_node(&package, members))
        .collect()
}

type Member<'a> = (usize, &'a Augmentation);

/// The package's header row. Its count is items; a package of lone marketplaces counts their
/// plugins.
fn group_node(package: &Package, members: Vec<Member>) -> Node {
    let (marketplaces, mut rest): (Vec<_>, Vec<_>) = members
        .iter()
        .copied()
        .partition(|(_, a)| a.kind == Kind::Marketplace);
    rest.sort_by(|(_, a), (_, b)| (a.kind, &a.name).cmp(&(b.kind, &b.name)));
    let (count, children) = if rest.is_empty() {
        marketplace_children(&marketplaces)
    } else {
        (
            rest.len(),
            rest.into_iter().map(|(i, a)| item_node(i, a)).collect(),
        )
    };
    let scope = members[0].1.scope;
    Node {
        label: group_label(package),
        cells: [
            String::new(),
            String::new(),
            scope.to_string(),
            count.to_string(),
        ],
        children,
        item: None,
        scope: Some(scope),
        ..Node::default()
    }
}

fn group_label(package: &Package) -> String {
    match package.is_marketplace() {
        true => format!("{} (marketplace)", package.title()),
        false => package.title(),
    }
}

/// Lone marketplaces show their plugins as leaves.
fn marketplace_children(marketplaces: &[Member]) -> (usize, Vec<Node>) {
    let count = marketplaces.iter().map(|(_, m)| m.children.len()).sum();
    let mut leaves: Vec<&String> = marketplaces.iter().flat_map(|(_, m)| &m.children).collect();
    leaves.sort();
    leaves.dedup();
    (count, leaves.into_iter().map(Node::leaf).collect())
}

/// Fill every item row's modal text from `info(item index)`, and each group's with a summary
/// of its members.
pub fn attach_info(nodes: &mut [Node], info: &dyn Fn(usize) -> Vec<String>) {
    for group in nodes {
        for child in &mut group.children {
            if let Some(i) = child.item {
                child.info = info(i);
            }
        }
        let scope = group.cells[2].clone();
        let members: Vec<String> = group
            .children
            .iter()
            .map(|c| format!("  {}", c.label))
            .collect();
        group.info = vec![format!("{} items ({scope})", group.cells[3])]
            .into_iter()
            .chain(members)
            .collect();
    }
}

/// One top-level node per label, each an item with the label's position as its index: the
/// shape a flat picker needs.
pub fn flat(labels: &[String]) -> Vec<Node> {
    labels
        .iter()
        .enumerate()
        .map(|(i, l)| Node {
            label: l.clone(),
            item: Some(i),
            ..Node::default()
        })
        .collect()
}

/// Every item row under `nodes` (preorder) with its `Node::item` index and label.
pub fn items(nodes: &[Node]) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for n in nodes {
        out.extend(n.item.map(|i| (i, n.label.clone())));
        out.extend(items(&n.children));
    }
    out
}

fn item_node(index: usize, item: &Augmentation) -> Node {
    Node {
        label: item.name.clone(),
        cells: [
            item.kind.to_string(),
            tag(&item.agents),
            item.scope.to_string(),
            String::new(),
        ],
        children: crate::mcp::shown_children(item)
            .iter()
            .map(Node::leaf)
            .collect(),
        item: Some(index),
        ..Node::default()
    }
}

/// Group headers only (their labels already carry the counts).
pub fn collapse(nodes: Vec<Node>) -> Vec<Node> {
    nodes
        .into_iter()
        .map(|n| Node {
            children: vec![],
            ..n
        })
        .collect()
}

#[cfg(test)]
mod tests;
