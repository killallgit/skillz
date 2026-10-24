use super::*;
use crate::model::Installer;
use crate::model::{Agent, Scope};

fn aug(name: &str, kind: Kind, group: &str) -> Installed {
    Installed {
        details: Default::default(),
        hooks: Vec::new(),
        members: Vec::new(),
        id: format!("{kind}:{name}"),
        name: name.into(),
        kind,
        agents: vec![Agent::Claude, Agent::Codex],
        scope: Scope::Global,
        group: group.into(),
        source: (group != "local" && matches!(kind, Kind::Skill | Kind::Subagent))
            .then(|| group.to_string()),
        installer: Installer::Local,
        origin: Default::default(),
        children: vec![],
        removal: vec![],
    }
}

fn sourced(item: Installed, source: &str) -> Installed {
    Installed {
        source: Some(source.into()),
        ..item
    }
}

fn with_children(item: Installed, children: &[&str]) -> Installed {
    Installed {
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
    // Each package's items by input index: marketplaces local and mp, then local files, hooks.
    let groups: Vec<Vec<Option<usize>>> = nodes
        .iter()
        .map(|g| g.children.iter().map(|n| n.item).collect())
        .collect();
    assert_eq!(
        groups,
        [
            vec![Some(4)],
            vec![Some(1)],
            vec![Some(2), Some(0)],
            vec![Some(3)]
        ]
    );
    assert_eq!(nodes[0].item, None);

    assert_eq!(nodes[1].children[0].children.len(), 2);
    assert!(nodes[1].children[0].children[0].children.is_empty());
    assert_eq!(nodes[1].children[0].children[0].item, None);
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
    // The project group leads, then global: named packages, then local files.
    let first_items: Vec<Option<usize>> = nodes.iter().map(|g| g.children[0].item).collect();
    assert_eq!(nodes[0].scope, Some(Scope::Project));
    assert_eq!(first_items, [Some(2), Some(0), Some(1)]);
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
fn a_merged_item_shows_its_agents_once_abbreviated_in_the_agents_column() {
    let mut item = aug("a", Kind::Skill, "local");
    item.agents = vec![Agent::Claude, Agent::Codex];

    let nodes = build(&[item]);

    let shown = &nodes[0].children[0].cells[1];
    assert_eq!(*shown, crate::merge::tag(&[Agent::Claude, Agent::Codex]));
}

#[test]
fn skills_and_subagents_of_one_source_share_a_package_and_say_their_kind() {
    let nodes = build(&[
        aug("tdd", Kind::Skill, "o/r"),
        aug("reviewer", Kind::Subagent, "o/r"),
    ]);

    let kinds: Vec<&str> = nodes[0]
        .children
        .iter()
        .map(|n| n.cells[0].as_str())
        .collect();
    assert_eq!((nodes.len(), kinds), (1, vec!["skill", "subagent"]));
}

#[test]
fn the_same_package_in_two_scopes_is_two_groups() {
    let mut global = aug("tdd", Kind::Skill, "o/r");
    global.scope = Scope::Global;
    let mut project = aug("tdd", Kind::Skill, "o/r");
    project.scope = Scope::Project;

    let nodes = build(&[global, project]);

    let scopes: Vec<Option<Scope>> = nodes.iter().map(|n| n.scope).collect();
    assert_eq!(scopes, [Some(Scope::Project), Some(Scope::Global)]);
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
        .zip(["mp", "p@mp", "s1", "s2", "q@mp", "s3", "hooks", "rtk"])
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

#[test]
fn picking_the_group_of_a_lone_marketplace_picks_the_marketplace() {
    let nodes = build(&[
        aug("local", Kind::Skill, "local"),
        with_children(aug("mp", Kind::Marketplace, "mp"), &["a", "b"]),
    ]);
    let group = nodes
        .iter()
        .position(|g| g.label.starts_with("mp"))
        .unwrap();
    let picked = crate::prompt::Picked {
        groups: vec![group],
        items: vec![],
    };

    assert_eq!(picked.item_indices(&nodes), [1]);
}

#[test]
fn a_group_with_item_rows_covers_nothing_extra() {
    let nodes = build(&[
        with_children(aug("p@mp", Kind::Plugin, "mp"), &["s1"]),
        with_children(aug("mp", Kind::Marketplace, "mp"), &["p"]),
    ]);

    assert!(nodes.iter().all(|g| g.covers.is_empty()));
}
