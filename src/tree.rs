use std::collections::BTreeMap;

use crate::model::{Augmentation, Kind, Scope};

mod table;

pub use table::{Widths, render};

/// Group of skills with no recorded source.
const LOCAL: &str = "local";
const LOCAL_LABEL: &str = "local files (no source)";

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

/// Group augmentations for display: scope -> group -> item -> item.children. Project groups
/// come first, then global; within a scope the local-files group leads. Sorted, deterministic.
pub fn build(items: &[Augmentation]) -> Vec<Node> {
    let mut groups: BTreeMap<(&str, &str), Vec<(usize, &Augmentation)>> = BTreeMap::new();
    for (i, item) in items.iter().enumerate() {
        // Keyed by scope name so BTreeMap order stays deterministic: "global" < "project".
        groups
            .entry((scope_key(item.scope), &item.group))
            .or_default()
            .push((i, item));
    }
    let mut groups: Vec<_> = groups.into_iter().collect();
    groups
        .sort_by_key(|((scope, group), _)| (*scope != scope_key(Scope::Project), *group != LOCAL));
    groups
        .into_iter()
        .map(|((_, group), members)| group_node(group, members))
        .collect()
}

fn scope_key(scope: Scope) -> &'static str {
    match scope {
        Scope::Global => "global",
        Scope::Project => "project",
    }
}

type Member<'a> = (usize, &'a Augmentation);

/// The group's header row. Its count is items; a group of lone marketplaces counts their plugins.
fn group_node(group: &str, members: Vec<Member>) -> Node {
    let (marketplaces, mut rest): (Vec<_>, Vec<_>) = members
        .iter()
        .copied()
        .partition(|(_, a)| a.kind == Kind::Marketplace && a.name == group);
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
        label: group_label(group, &members, !marketplaces.is_empty()),
        cells: [
            String::new(),
            String::new(),
            scope_key(scope).into(),
            count.to_string(),
        ],
        children,
        item: None,
        scope: Some(scope),
        ..Node::default()
    }
}

fn group_label(group: &str, members: &[Member], marketplace: bool) -> String {
    // Skills with no recorded source: plain folders someone put in a skills dir.
    let group = if group == LOCAL { LOCAL_LABEL } else { group };
    let source = members
        .iter()
        .filter_map(|(_, a)| a.source.as_deref())
        .find(|s| *s != group)
        .unwrap_or(group);
    let suffix = if marketplace { " (marketplace)" } else { "" };
    format!("{source}{suffix}")
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
    let agents: Vec<String> = item.agents.iter().map(|a| a.to_string()).collect();
    Node {
        label: item.name.clone(),
        cells: [
            item.kind.to_string(),
            agents.join(","),
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
mod tests {
    use super::*;
    use crate::model::Installer;
    use crate::model::{Agent, Scope};

    fn aug(name: &str, kind: Kind, group: &str) -> Augmentation {
        Augmentation {
            hooks: Vec::new(),
            members: Vec::new(),
            id: format!("{kind}:{name}"),
            name: name.into(),
            kind,
            agents: vec![Agent::Claude, Agent::Codex],
            scope: Scope::Global,
            group: group.into(),
            source: None,
            installer: Installer::Local,
            origin: Default::default(),
            children: vec![],
            removal: vec![],
        }
    }

    fn sourced(item: Augmentation, source: &str) -> Augmentation {
        Augmentation {
            source: Some(source.into()),
            ..item
        }
    }

    fn with_children(item: Augmentation, children: &[&str]) -> Augmentation {
        Augmentation {
            children: children.iter().map(|c| c.to_string()).collect(),
            ..item
        }
    }

    #[test]
    fn groups_sorted_with_items_sorted_by_kind_then_name() {
        let nodes = build(&[
            aug("zeta", Kind::Skill, "local"),
            with_children(aug("p@mp", Kind::Plugin, "mp"), &["s1", "s2"]),
            aug("alpha", Kind::Skill, "local"),
            with_children(aug("rtk", Kind::Hook, "hooks"), &["PreToolUse: rtk"]),
            aug("beta", Kind::Plugin, "local"),
        ]);
        // Each group's items by input index: local (beta, alpha, zeta), hooks, then mp.
        let groups: Vec<Vec<Option<usize>>> = nodes
            .iter()
            .map(|g| g.children.iter().map(|n| n.item).collect())
            .collect();
        assert_eq!(
            groups,
            [
                vec![Some(4), Some(2), Some(0)],
                vec![Some(3)],
                vec![Some(1)]
            ]
        );
        assert_eq!(nodes[0].item, None);

        assert_eq!(nodes[2].children[0].children.len(), 2);
        assert!(nodes[2].children[0].children[0].children.is_empty());
        assert_eq!(nodes[2].children[0].children[0].item, None);
    }

    #[test]
    fn source_replaces_group_name_when_it_differs() {
        let same = build(&[sourced(aug("a", Kind::Skill, "org/repo"), "org/repo")]);
        assert!(same[0].label.starts_with("org/repo"));
        let diff = build(&[sourced(aug("a", Kind::Skill, "mp"), "org/repo")]);
        assert!(diff[0].label.starts_with("org/repo"));
    }

    #[test]
    fn headers_align_and_count_items() {
        let mut p = aug("b", Kind::Skill, "long-group-name");
        p.scope = Scope::Project;
        let nodes = build(&[aug("a", Kind::Skill, "g"), aug("c", Kind::Skill, "g"), p]);
        assert_eq!(nodes[0].cells[3], "1");
        assert_eq!(nodes[1].cells[3], "2");
        // Scope and count columns start at the same offset on every row, header included.
        let text = render(&collapse(nodes.clone()));
        let at = |l: &str, w: &str| l.find(w).unwrap();
        let lines: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(at(lines[0], "SCOPE"), at(lines[1], "project"));
        assert_eq!(at(lines[0], "SCOPE"), at(lines[2], "global"));
    }

    #[test]
    fn project_groups_come_first_then_a_gap_then_global() {
        let mut proj = aug("p", Kind::Skill, "zzz");
        proj.scope = Scope::Project;
        let nodes = build(&[
            aug("a", Kind::Skill, "aaa"),
            aug("b", Kind::Skill, "local"),
            proj,
        ]);
        // The project group leads, then the local-files group, then the rest, by input index.
        let first_items: Vec<Option<usize>> = nodes.iter().map(|g| g.children[0].item).collect();
        assert_eq!(nodes[0].scope, Some(Scope::Project));
        assert_eq!(first_items, [Some(2), Some(1), Some(0)]);
        assert!(render(&nodes).contains("\n\n"));
    }

    #[test]
    fn marketplace_folds_into_its_group() {
        let nodes = build(&[
            with_children(
                sourced(aug("mp", Kind::Marketplace, "mp"), "org/repo"),
                &["x", "y"],
            ),
            with_children(aug("p@mp", Kind::Plugin, "mp"), &["s"]),
        ]);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].children.len(), 1);
        assert_eq!(nodes[0].children[0].item, Some(1));
    }

    #[test]
    fn long_leaves_are_capped() {
        let long = "x".repeat(500);
        assert!(Node::leaf(long.as_str()).label.chars().count() < 500);
        assert_eq!(Node::leaf("short").label, "short");
    }

    #[test]
    fn lone_marketplace_shows_its_children() {
        let nodes = build(&[
            with_children(aug("mp", Kind::Marketplace, "mp"), &["b", "a"]),
            with_children(aug("mp", Kind::Marketplace, "mp"), &["a"]),
        ]);
        assert_eq!(nodes.len(), 1);
        let kids: Vec<&str> = nodes[0].children.iter().map(|n| n.label.as_str()).collect();
        assert_eq!(kids, ["a", "b"]);
    }

    #[test]
    fn collapse_keeps_one_childless_node_per_group() {
        let nodes = build(&[
            with_children(aug("p@mp", Kind::Plugin, "mp"), &["s1", "s2"]),
            with_children(aug("rtk", Kind::Hook, "hooks"), &["PreToolUse: rtk"]),
        ]);
        let collapsed = collapse(nodes.clone());
        assert_eq!(collapsed.len(), nodes.len());
        assert!(collapsed.iter().all(|n| n.children.is_empty()));
        assert_eq!(render(&collapsed).lines().count(), 3);
    }

    #[test]
    fn empty_input_builds_and_renders_nothing() {
        assert!(build(&[]).is_empty());
        assert_eq!(render(&[]), "");
    }

    #[test]
    fn render_emits_one_line_per_node() {
        let nodes = build(&[
            with_children(aug("p@mp", Kind::Plugin, "mp"), &["s1", "s2"]),
            with_children(aug("q@mp", Kind::Plugin, "mp"), &["s3"]),
            aug("rtk", Kind::Hook, "hooks"),
        ]);
        let text = render(&nodes);
        assert!(text.ends_with('\n'));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 9);
        for (line, name) in lines[1..]
            .iter()
            .zip(["hooks", "rtk", "mp", "p@mp", "s1", "s2", "q@mp", "s3"])
        {
            assert!(line.contains(name), "{line:?} lacks {name}");
        }
    }

    #[test]
    fn info_is_attached_to_items_and_summarises_groups() {
        let items = [with_children(aug("a", Kind::Skill, "g"), &["detail"])];
        let mut nodes = build(&items);
        attach_info(&mut nodes, &|i| vec![format!("item {i}")]);
        assert_eq!(nodes[0].children[0].info, ["item 0"]);
        assert_eq!(nodes[0].info.len(), 2);
        // Detail leaves carry no info, so `i` opens no modal on them.
        assert!(nodes[0].children[0].children[0].info.is_empty());
    }
}
