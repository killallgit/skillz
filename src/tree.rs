use std::collections::BTreeMap;

use crate::model::{Augmentation, Kind};

/// A display tree node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub label: String,
    pub children: Vec<Node>,
}

impl Node {
    fn leaf(label: impl Into<String>) -> Self {
        Node {
            label: label.into(),
            children: vec![],
        }
    }
}

/// Group augmentations for display: group -> item -> item.children. Sorted, deterministic.
pub fn build(items: &[Augmentation]) -> Vec<Node> {
    let mut groups: BTreeMap<&str, Vec<&Augmentation>> = BTreeMap::new();
    for item in items {
        groups.entry(&item.group).or_default().push(item);
    }
    groups
        .into_iter()
        .map(|(group, members)| group_node(group, members))
        .collect()
}

fn group_node(group: &str, members: Vec<&Augmentation>) -> Node {
    let (marketplaces, mut rest): (Vec<&Augmentation>, Vec<&Augmentation>) = members
        .iter()
        .copied()
        .partition(|a| a.kind == Kind::Marketplace && a.name == group);
    rest.sort_by(|a, b| (a.kind, &a.name).cmp(&(b.kind, &b.name)));

    let mut label = group.to_string();
    if let Some(src) = members
        .iter()
        .filter_map(|a| a.source.as_deref())
        .find(|s| *s != group)
    {
        label.push_str(&format!(" [{src}]"));
    }
    if !marketplaces.is_empty() {
        label.push_str(" (marketplace)");
    }

    let children = if rest.is_empty() {
        let mut leaves: Vec<&String> = marketplaces.iter().flat_map(|m| &m.children).collect();
        leaves.sort();
        leaves.dedup();
        leaves.into_iter().map(Node::leaf).collect()
    } else {
        rest.into_iter().map(item_node).collect()
    };
    Node { label, children }
}

fn item_node(item: &Augmentation) -> Node {
    let agents: Vec<String> = item.agents.iter().map(ToString::to_string).collect();
    Node {
        label: format!(
            "{}  ({} · {} · {})",
            item.name,
            item.kind,
            agents.join(","),
            item.scope
        ),
        children: item.children.iter().map(Node::leaf).collect(),
    }
}

/// Render as a box-drawing text tree (`├──`, `└──`, `│`).
pub fn render(nodes: &[Node]) -> String {
    let mut out = String::new();
    for node in nodes {
        out.push_str(&node.label);
        out.push('\n');
        render_children(&node.children, "", &mut out);
    }
    out
}

fn render_children(nodes: &[Node], prefix: &str, out: &mut String) {
    for (i, node) in nodes.iter().enumerate() {
        let last = i + 1 == nodes.len();
        let (branch, indent) = if last {
            ("└── ", "    ")
        } else {
            ("├── ", "│   ")
        };
        out.push_str(&format!("{prefix}{branch}{}\n", node.label));
        render_children(&node.children, &format!("{prefix}{indent}"), out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert!(tops[0].starts_with("hooks"));
        assert!(tops[1].starts_with("local"));
        assert!(tops[2].starts_with("mp"));

        let local = &nodes[1].children;
        assert_eq!(local.len(), 3);
        assert!(local[0].label.starts_with("beta"));
        assert!(local[1].label.starts_with("alpha"));
        assert!(local[2].label.starts_with("zeta"));

        assert_eq!(nodes[2].children[0].children.len(), 2);
        assert!(nodes[2].children[0].children[0].children.is_empty());
    }

    #[test]
    fn source_decoration_only_when_it_differs_from_group() {
        let same = build(&[aug("a", Kind::Skill, "org/repo", Some("org/repo"), &[])]);
        assert_eq!(same[0].label, "org/repo");
        let diff = build(&[aug("a", Kind::Skill, "mp", Some("org/repo"), &[])]);
        assert!(diff[0].label.starts_with("mp") && diff[0].label != "mp");
    }

    #[test]
    fn marketplace_folds_into_its_group() {
        let nodes = build(&[
            aug("mp", Kind::Marketplace, "mp", Some("org/repo"), &["x", "y"]),
            aug("p@mp", Kind::Plugin, "mp", None, &["s"]),
        ]);
        assert_eq!(nodes.len(), 1);
        assert_ne!(nodes[0].label, "mp");
        assert_eq!(nodes[0].children.len(), 1);
        assert!(nodes[0].children[0].label.starts_with("p@mp"));
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
        assert_eq!(lines.len(), 8);
        for (line, name) in lines
            .iter()
            .zip(["hooks", "rtk", "mp", "p@mp", "s1", "s2", "q@mp", "s3"])
        {
            assert!(line.contains(name), "{line:?} lacks {name}");
        }
    }
}
