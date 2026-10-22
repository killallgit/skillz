use super::*;
use crate::library::LibraryItem;
use crate::location::detail;
use crate::merge::tag;
use crate::model::{Agent, Installer, Kind, SKILL_CHILD, Scope};

pub fn installed(name: &str, kind: Kind, source: Option<&str>) -> Augmentation {
    Augmentation {
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

pub fn global(name: &str, kind: Kind, source: Option<&str>) -> Augmentation {
    Augmentation {
        details: Default::default(),
        id: format!("global-{kind}:{name}"),
        scope: Scope::Global,
        ..installed(name, kind, source)
    }
}

pub fn library_skill(source: &str, name: &str) -> LibraryItem {
    LibraryItem::from_entry(Entry::Skill {
        source: source.into(),
        names: vec![name.into()],
        agents: vec![],
    })
}

fn rows_of(here: &[Augmentation], also_global: &[Augmentation], library: &Catalog) -> Vec<Row> {
    build(&Sources {
        here,
        also_global,
        library,
    })
}

fn items(rows: &[Row]) -> Vec<&Item> {
    rows.iter()
        .filter_map(|row| match row {
            Row::Item(item) => Some(item),
            _ => None,
        })
        .collect()
}

fn packages_of(rows: &[Row], wanted: Part) -> Vec<Package> {
    rows.iter()
        .filter_map(|row| match row {
            Row::Package(group) if group.part == wanted => Some(group.package.clone()),
            _ => None,
        })
        .collect()
}

fn parents(rows: &[Row]) -> Vec<Option<usize>> {
    shape(rows).into_iter().map(|(parent, _)| parent).collect()
}

#[test]
fn installed_groups_come_first_then_one_available_header_with_its_own_groups() {
    let here = [installed("tdd", Kind::Skill, Some("o/r"))];
    let library = Catalog::from_items(vec![library_skill("o/r", "zoom")]);

    let rows = rows_of(&here, &[], &library);

    let header = rows.iter().position(|r| matches!(r, Row::Available { .. }));
    assert_eq!(header, Some(2));
    assert_eq!(rows.len(), 5);
    assert_eq!(
        packages_of(&rows, Part::Installed),
        [Package::Source("o/r".into())]
    );
    assert_eq!(
        packages_of(&rows, Part::Available),
        [Package::Source("o/r".into())]
    );
}

#[test]
fn library_items_already_installed_here_are_not_available() {
    let here = [installed("tdd", Kind::Skill, Some("o/r"))];
    let library = Catalog::from_items(vec![library_skill("o/r", "tdd")]);

    let rows = rows_of(&here, &[], &library);

    assert!(!rows.iter().any(|r| matches!(r, Row::Available { .. })));
    assert_eq!(items(&rows).len(), 1);
}

#[test]
fn an_item_installed_globally_as_well_is_tagged_whether_installed_or_available() {
    let here = [installed("tdd", Kind::Skill, Some("o/r"))];
    let elsewhere = [
        global("tdd", Kind::Skill, Some("o/r")),
        global("grill", Kind::Skill, Some("o/r")),
    ];
    let library = Catalog::from_items(vec![
        library_skill("o/r", "grill"),
        library_skill("o/r", "zoom"),
    ]);

    let rows = rows_of(&here, &elsewhere, &library);

    let tagged: Vec<bool> = items(&rows).iter().map(|item| item.global).collect();
    assert_eq!(tagged, [true, true, false]);
}

#[test]
fn skills_and_subagents_of_one_repo_are_one_group() {
    let here = [
        installed("tdd", Kind::Skill, Some("o/r")),
        installed("reviewer", Kind::Subagent, Some("o/r")),
        installed("grill", Kind::Skill, Some("o/r")),
    ];

    let rows = rows_of(&here, &[], &Catalog::default());

    assert_eq!(packages_of(&rows, Part::Installed).len(), 1);
    assert_eq!(parents(&rows), [None, Some(0), Some(0), Some(0)]);
}

#[test]
fn items_in_a_group_are_ordered_by_kind_then_name() {
    let here = [
        installed("reviewer", Kind::Subagent, Some("o/r")),
        installed("zoom", Kind::Skill, Some("o/r")),
        installed("Alpha", Kind::Skill, Some("o/r")),
    ];

    let rows = rows_of(&here, &[], &Catalog::default());

    let kinds: Vec<ItemKind> = items(&rows).iter().map(|i| i.kind).collect();
    let first_skill = &items(&rows)[0].state;
    assert_eq!(
        kinds,
        [ItemKind::Skill, ItemKind::Skill, ItemKind::Subagent]
    );
    assert_eq!(*first_skill, State::Installed("skill:Alpha".into()));
}

#[test]
fn the_same_skill_from_two_sources_is_in_two_groups() {
    let here = [
        installed("tdd", Kind::Skill, Some("a/r")),
        installed("tdd", Kind::Skill, Some("b/r")),
    ];

    let rows = rows_of(&here, &[], &Catalog::default());

    assert_eq!(packages_of(&rows, Part::Installed).len(), 2);
}

#[test]
fn local_files_mcp_servers_and_unowned_hooks_follow_the_named_packages() {
    let here = [
        installed("h", Kind::Hook, None),
        installed("m", Kind::Mcp, None),
        installed("mine", Kind::Skill, None),
        installed("tdd", Kind::Skill, Some("o/r")),
    ];

    let rows = rows_of(&here, &[], &Catalog::default());

    assert_eq!(
        packages_of(&rows, Part::Installed),
        [
            Package::Source("o/r".into()),
            Package::Local,
            Package::Mcp,
            Package::Hooks
        ]
    );
}

#[test]
fn a_marketplace_is_the_package_of_its_plugins_and_has_no_row_of_its_own() {
    let on_marketplace = |kind, name| Augmentation {
        group: "mp".into(),
        ..installed(name, kind, Some("o/mp"))
    };
    let here = [
        on_marketplace(Kind::Marketplace, "mp"),
        on_marketplace(Kind::Plugin, "p@mp"),
    ];

    let rows = rows_of(&here, &[], &Catalog::default());

    assert_eq!(
        packages_of(&rows, Part::Installed),
        [Package::Marketplace("mp".into())]
    );
    assert_eq!(items(&rows).len(), 1);
}

fn plugin_providing_a_skill_and_a_hook(name: &str) -> Augmentation {
    Augmentation {
        details: crate::model::Details::Plugin {
            location: Default::default(),
            skills: vec!["tdd".into()],
        },
        hooks: vec![crate::hook_text::HookEntry {
            event: "Stop".into(),
            matcher: None,
            command: "say done".into(),
        }],
        children: vec![format!("{SKILL_CHILD}tdd")],
        ..installed(name, Kind::Plugin, Some("mp"))
    }
}

#[test]
fn a_plugin_is_followed_by_the_skills_and_hooks_it_provides_not_the_hooks_group() {
    let here = [plugin_providing_a_skill_and_a_hook("p@mp")];

    let rows = rows_of(&here, &[], &Catalog::default());

    assert!(matches!(
        rows.as_slice(),
        [Row::Package(_), Row::Item(_), Row::Owned(_), Row::Owned(_)]
    ));
    assert_eq!(packages_of(&rows, Part::Installed).len(), 1);
}

#[test]
fn provided_rows_hang_under_their_plugin_and_say_which_plugin_adds_them() {
    let here = [plugin_providing_a_skill_and_a_hook("p@mp")];

    let rows = rows_of(&here, &[], &Catalog::default());

    assert_eq!(parents(&rows), [None, Some(0), Some(1), Some(1)]);
    let kinds: Vec<ItemKind> = rows
        .iter()
        .filter_map(|row| match row {
            Row::Owned(owned) => Some(owned.kind),
            _ => None,
        })
        .collect();
    assert_eq!(kinds, [ItemKind::Skill, ItemKind::Hook]);
}

#[test]
fn installed_and_available_groups_of_one_package_have_different_labels() {
    let here = [installed("a", Kind::Skill, Some("o/r"))];
    let library = Catalog::from_items(vec![library_skill("o/r", "zoom")]);

    let shape = shape(&rows_of(&here, &[], &library));

    assert_ne!(shape[0].1, shape[3].1);
    assert_eq!(
        parents(&rows_of(&here, &[], &library)),
        [None, Some(0), None, Some(2), Some(3)]
    );
}

fn subagent_in(agent: Agent) -> Augmentation {
    Augmentation {
        details: Default::default(),
        id: format!("subagent:{agent}:reviewer"),
        agents: vec![agent],
        ..installed("reviewer", Kind::Subagent, None)
    }
}

fn merged_subagent() -> Augmentation {
    crate::merge::merge(vec![subagent_in(Agent::Claude), subagent_in(Agent::Codex)]).remove(0)
}

#[test]
fn a_thing_seen_through_two_agents_is_one_row_counted_once() {
    let here = [merged_subagent()];

    let rows = rows_of(&here, &[], &Catalog::default());

    assert_eq!(items(&rows).len(), 1);
}

#[test]
fn a_merged_row_names_its_agents_and_every_location() {
    let here = [merged_subagent()];

    let rows = rows_of(&here, &[], &Catalog::default());

    let row = items(&rows)[0];
    assert_eq!(row.agents, tag(&[Agent::Claude, Agent::Codex]));
    assert_eq!(row.detail, detail(&here[0]));
    assert_eq!(
        row.state,
        State::Installed("subagent:claude:reviewer".into())
    );
}

#[test]
fn an_available_row_has_no_agents_tag() {
    let library = Catalog::from_items(vec![library_skill("o/r", "zoom")]);

    let rows = rows_of(&[], &[], &library);

    assert!(items(&rows)[0].agents.is_empty());
}
