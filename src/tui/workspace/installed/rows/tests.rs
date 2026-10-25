use super::*;
use crate::location::detail;
use crate::merge::tag;
use skillz_harness::model::{Agent, Installer, Kind, Scope};

pub fn installed(name: &str, kind: Kind, source: Option<&str>) -> Installed {
    Installed {
        details: Default::default(),
        hooks: vec![],
        members: Vec::new(),
        id: format!("{kind}:{name}"),
        name: name.into(),
        kind,
        agents: vec![Agent::Claude, Agent::Codex],
        scope: Scope::Project,
        group: source.unwrap_or("local").into(),
        source: source.map(Into::into),
        installer: Installer::Npx,
        origin: Default::default(),
        children: vec![],
        removal: vec![],
    }
}

pub fn global(name: &str, kind: Kind, source: Option<&str>) -> Installed {
    Installed {
        details: Default::default(),
        id: format!("global-{kind}:{name}"),
        scope: Scope::Global,
        ..installed(name, kind, source)
    }
}

pub fn plugin_providing_a_skill_and_a_hook(name: &str) -> Installed {
    Installed {
        details: skillz_harness::model::Details::Plugin {
            location: Default::default(),
            skills: vec!["tdd".into()],
        },
        hooks: vec![skillz_harness::hook_text::HookEntry {
            event: "Stop".into(),
            matcher: None,
            command: "say done".into(),
        }],
        children: vec![format!("{}tdd", skillz_harness::model::SKILL_CHILD)],
        ..installed(name, Kind::Plugin, Some("mp"))
    }
}

fn items(rows: &[Row]) -> Vec<&Item> {
    rows.iter()
        .filter_map(|row| match row {
            Row::Item(item) => Some(item),
            _ => None,
        })
        .collect()
}

fn sections(rows: &[Row]) -> Vec<ItemKind> {
    rows.iter()
        .filter_map(|row| match row {
            Row::Section(kind) => Some(*kind),
            _ => None,
        })
        .collect()
}

fn packages(rows: &[Row]) -> Vec<Package> {
    rows.iter()
        .filter_map(|row| match row {
            Row::Package(group) => Some(group.package.clone()),
            _ => None,
        })
        .collect()
}

fn parents(rows: &[Row]) -> Vec<Option<usize>> {
    shape(rows).into_iter().map(|(parent, _)| parent).collect()
}

#[test]
fn a_section_is_shown_for_each_kind_the_scope_has_in_a_fixed_order() {
    let here = [
        installed("h", Kind::Hook, None),
        installed("m", Kind::Mcp, None),
        installed("reviewer", Kind::Subagent, None),
        installed("tdd", Kind::Skill, Some("o/r")),
        plugin_providing_a_skill_and_a_hook("p@mp"),
    ];

    let rows = build(&here);

    assert_eq!(
        sections(&rows),
        [
            ItemKind::Plugin,
            ItemKind::Skill,
            ItemKind::Subagent,
            ItemKind::Mcp,
            ItemKind::Hook
        ]
    );
}

#[test]
fn a_kind_with_nothing_installed_has_no_section() {
    let here = [installed("tdd", Kind::Skill, Some("o/r"))];

    let rows = build(&here);

    assert_eq!(sections(&rows), [ItemKind::Skill]);
    assert!(build(&[]).is_empty());
}

#[test]
fn skills_of_one_repo_are_one_package_under_the_skills_section() {
    let here = [
        installed("tdd", Kind::Skill, Some("o/r")),
        installed("grill", Kind::Skill, Some("o/r")),
        installed("mine", Kind::Skill, None),
    ];

    let rows = build(&here);

    assert_eq!(
        packages(&rows),
        [Package::Source("o/r".into()), Package::Local]
    );
    assert_eq!(
        parents(&rows),
        [None, Some(0), Some(1), Some(1), Some(0), Some(4)]
    );
}

#[test]
fn items_in_a_package_are_ordered_by_name() {
    let here = [
        installed("zoom", Kind::Skill, Some("o/r")),
        installed("Alpha", Kind::Skill, Some("o/r")),
    ];

    let rows = build(&here);

    let labels: Vec<&str> = items(&rows).iter().map(|i| i.label.as_str()).collect();
    assert_eq!(labels, ["Alpha", "zoom"]);
}

#[test]
fn the_same_skill_from_two_sources_is_in_two_packages() {
    let here = [
        installed("tdd", Kind::Skill, Some("a/r")),
        installed("tdd", Kind::Skill, Some("b/r")),
    ];

    assert_eq!(packages(&build(&here)).len(), 2);
}

#[test]
fn agents_mcp_servers_and_hooks_hang_straight_under_their_section() {
    let here = [
        installed("reviewer", Kind::Subagent, Some("o/r")),
        installed("m", Kind::Mcp, None),
    ];

    let rows = build(&here);

    assert!(packages(&rows).is_empty());
    assert_eq!(parents(&rows), [None, Some(0), None, Some(2)]);
}

#[test]
fn a_plugin_is_one_row_that_says_what_it_provides_and_lists_none_of_it() {
    let here = [plugin_providing_a_skill_and_a_hook("p@mp")];

    let rows = build(&here);

    assert!(matches!(
        rows.as_slice(),
        [Row::Section(ItemKind::Plugin), Row::Item(_)]
    ));
    assert_eq!(items(&rows)[0].detail, "1 skill, 1 hook");
}

#[test]
fn a_marketplace_has_no_row_of_its_own() {
    let on_marketplace = |kind, name| Installed {
        group: "mp".into(),
        ..installed(name, kind, Some("o/mp"))
    };
    let here = [
        on_marketplace(Kind::Marketplace, "mp"),
        on_marketplace(Kind::Plugin, "p@mp"),
    ];

    assert_eq!(items(&build(&here)).len(), 1);
}

fn subagent_in(agent: Agent) -> Installed {
    Installed {
        details: Default::default(),
        id: format!("subagent:{agent}:reviewer"),
        agents: vec![agent],
        ..installed("reviewer", Kind::Subagent, None)
    }
}

fn merged_subagent() -> Installed {
    crate::merge::merge(vec![subagent_in(Agent::Claude), subagent_in(Agent::Codex)]).remove(0)
}

#[test]
fn a_thing_seen_through_two_agents_is_one_row_naming_its_agents_and_locations() {
    let here = [merged_subagent()];

    let rows = build(&here);

    let row = items(&rows)[0];
    assert_eq!(items(&rows).len(), 1);
    assert_eq!(row.agents, tag(&[Agent::Claude, Agent::Codex]));
    assert_eq!(row.detail, detail(&here[0]));
    assert_eq!(row.id, "subagent:claude:reviewer");
}

#[test]
fn a_section_and_a_package_count_the_items_beneath_them() {
    let here = [
        installed("tdd", Kind::Skill, Some("o/r")),
        installed("grill", Kind::Skill, Some("o/r")),
        installed("mine", Kind::Skill, None),
    ];
    let rows = build(&here);

    let counts: Vec<usize> = [0, 1, 4]
        .into_iter()
        .map(|at| items_of(&rows, at).count())
        .collect();

    assert_eq!(counts, [3, 2, 1]);
}
