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

/// The group's header row. Its count is items; a group of lone marketplaces counts their plugins.
fn group_node(group: &str, members: Vec<(usize, &Augmentation)>) -> Node {
    let (marketplaces, mut rest): (Vec<_>, Vec<_>) = members
        .iter()
        .copied()
        .partition(|(_, a)| a.kind == Kind::Marketplace && a.name == group);
    rest.sort_by(|(_, a), (_, b)| (a.kind, &a.name).cmp(&(b.kind, &b.name)));

    // Skills with no recorded source: plain folders someone put in a skills dir.
    let group = if group == LOCAL { LOCAL_LABEL } else { group };
    let mut label = members
        .iter()
        .filter_map(|(_, a)| a.source.as_deref())
        .find(|s| *s != group)
        .unwrap_or(group)
        .to_string();
    if !marketplaces.is_empty() {
        label.push_str(" (marketplace)");
    }

    let count = if rest.is_empty() {
        marketplaces.iter().map(|(_, m)| m.children.len()).sum()
    } else {
        rest.len()
    };
    let children = if rest.is_empty() {
        let mut leaves: Vec<&String> = marketplaces.iter().flat_map(|(_, m)| &m.children).collect();
        leaves.sort();
        leaves.dedup();
        leaves.into_iter().map(Node::leaf).collect()
    } else {
        rest.into_iter().map(|(i, a)| item_node(i, a)).collect()
    };
    let scope = members[0].1.scope;
    Node {
        label,
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
        children: item.children.iter().map(Node::leaf).collect(),
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

    fn aug(
        name: &str,
        kind: Kind,
        group: &str,
        source: Option<&str>,
        children: &[&str],
    ) -> Augmentation {
        Augmentation {
            id: format!("{kind}:{name}"),
            name: name.into(),
            kind,
            agents: vec![Agent::Claude, Agent::Codex],
            scope: Scope::Global,
            group: group.into(),
            source: source.map(Into::into),
            installer: Installer::Local,
            children: children.iter().map(|c| c.to_string()).collect(),
            removal: vec![],
        }
    }

    #[test]
    fn groups_sorted_with_items_sorted_by_kind_then_name() {
        let nodes = build(&[
            aug("zeta", Kind::Skill, "local", None, &[]),
            aug("p@mp", Kind::Plugin, "mp", None, &["s1", "s2"]),
            aug("alpha", Kind::Skill, "local", None, &[]),
            aug("rtk", Kind::Hook, "hooks", None, &["PreToolUse: rtk"]),
            aug("beta", Kind::Plugin, "local", None, &[]),
        ]);
        let tops: Vec<&str> = nodes.iter().map(|n| n.label.as_str()).collect();
        assert_eq!(tops.len(), 3);
        assert!(tops[0].starts_with("local"));
        assert!(tops[1].starts_with("hooks"));
        assert!(tops[2].starts_with("mp"));

        let local = &nodes[0].children;
        assert_eq!(local.len(), 3);
        assert!(local[0].label.starts_with("beta"));
        assert!(local[1].label.starts_with("alpha"));
        assert!(local[2].label.starts_with("zeta"));
        let items: Vec<Option<usize>> = local.iter().map(|n| n.item).collect();
        assert_eq!(items, [Some(4), Some(2), Some(0)]);
        assert_eq!(nodes[0].item, None);

        assert_eq!(nodes[2].children[0].children.len(), 2);
        assert!(nodes[2].children[0].children[0].children.is_empty());
        assert_eq!(nodes[2].children[0].children[0].item, None);
    }

    #[test]
    fn source_replaces_group_name_when_it_differs() {
        let same = build(&[aug("a", Kind::Skill, "org/repo", Some("org/repo"), &[])]);
        assert!(same[0].label.starts_with("org/repo"));
        let diff = build(&[aug("a", Kind::Skill, "mp", Some("org/repo"), &[])]);
        assert!(diff[0].label.starts_with("org/repo"));
    }

    #[test]
    fn headers_align_and_count_items() {
        let mut p = aug("b", Kind::Skill, "long-group-name", None, &[]);
        p.scope = Scope::Project;
        let nodes = build(&[
            aug("a", Kind::Skill, "g", None, &[]),
            aug("c", Kind::Skill, "g", None, &[]),
            p,
        ]);
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
        let mut proj = aug("p", Kind::Skill, "zzz", None, &[]);
        proj.scope = Scope::Project;
        let nodes = build(&[
            aug("a", Kind::Skill, "aaa", None, &[]),
            aug("b", Kind::Skill, "local", None, &[]),
            proj,
        ]);
        assert_eq!(nodes[0].scope, Some(Scope::Project));
        assert!(nodes[1].label.starts_with(LOCAL_LABEL));
        assert!(nodes[2].label.starts_with("aaa"));
        assert!(render(&nodes).contains("\n\nlocal files"));
    }

    #[test]
    fn marketplace_folds_into_its_group() {
        let nodes = build(&[
            aug("mp", Kind::Marketplace, "mp", Some("org/repo"), &["x", "y"]),
            aug("p@mp", Kind::Plugin, "mp", None, &["s"]),
        ]);
        assert_eq!(nodes.len(), 1);
        assert!(nodes[0].label.starts_with("org/repo (marketplace)"));
        assert_eq!(nodes[0].children.len(), 1);
        assert!(nodes[0].children[0].label.starts_with("p@mp"));
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
            aug("mp", Kind::Marketplace, "mp", None, &["b", "a"]),
            aug("mp", Kind::Marketplace, "mp", None, &["a"]),
        ]);
        assert_eq!(nodes.len(), 1);
        let kids: Vec<&str> = nodes[0].children.iter().map(|n| n.label.as_str()).collect();
        assert_eq!(kids, ["a", "b"]);
    }

    #[test]
    fn collapse_keeps_one_childless_node_per_group() {
        let nodes = build(&[
            aug("p@mp", Kind::Plugin, "mp", None, &["s1", "s2"]),
            aug("rtk", Kind::Hook, "hooks", None, &["PreToolUse: rtk"]),
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
            aug("p@mp", Kind::Plugin, "mp", None, &["s1", "s2"]),
            aug("q@mp", Kind::Plugin, "mp", None, &["s3"]),
            aug("rtk", Kind::Hook, "hooks", None, &[]),
        ]);
        let text = render(&nodes);
        assert!(text.ends_with('\n'));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 9);
        assert!(lines[0].starts_with("NAME"));
        for (line, name) in lines[1..]
            .iter()
            .zip(["hooks", "rtk", "mp", "p@mp", "s1", "s2", "q@mp", "s3"])
        {
            assert!(line.contains(name), "{line:?} lacks {name}");
        }
    }

    #[test]
    fn info_is_attached_to_items_and_summarises_groups() {
        let items = [aug("a", Kind::Skill, "g", None, &["detail"])];
        let mut nodes = build(&items);
        attach_info(&mut nodes, &|i| vec![format!("item {i}")]);
        assert_eq!(nodes[0].children[0].info, ["item 0"]);
        assert!(nodes[0].info[0].starts_with("1 items"));
        assert!(nodes[0].info.iter().any(|l| l.contains('a')));
        // Detail leaves carry no info, so `i` opens no modal on them.
        assert!(nodes[0].children[0].children[0].info.is_empty());
    }
}
