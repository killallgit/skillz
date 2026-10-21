use super::*;
use crate::model::{Agent, Installer, Kind, Scope};

pub fn installed(name: &str, kind: Kind, source: Option<&str>) -> Augmentation {
    Augmentation {
        hooks: vec![],
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

fn rows_of(project: &[Augmentation], global: &[Augmentation], library: &Catalog) -> Vec<Row> {
    build(&Sources {
        project,
        global,
        library,
    })
}

fn groups_of(rows: &[Row]) -> Vec<(Group, usize)> {
    rows.iter()
        .filter_map(|row| match row {
            Row::Group { group, total } => Some((*group, *total)),
            _ => None,
        })
        .collect()
}

fn items_in(rows: &[Row], wanted: Group) -> Vec<&Item> {
    rows.iter()
        .filter_map(|row| match row {
            Row::Item(item) if item.group == wanted => Some(item),
            _ => None,
        })
        .collect()
}

#[test]
fn project_global_and_available_items_each_get_their_own_group() {
    let project = [installed("tdd", Kind::Skill, Some("o/r"))];
    let global = [global("grill", Kind::Skill, Some("o/r"))];
    let library = Catalog::from_items(vec![library_skill("o/r", "zoom")]);

    let rows = rows_of(&project, &global, &library);

    let groups = [
        (Group::Project, 1),
        (Group::Global, 1),
        (Group::Available, 1),
    ];
    assert_eq!(groups_of(&rows), groups);
}

#[test]
fn an_item_installed_in_both_places_is_in_both_groups() {
    let project = [installed("tdd", Kind::Skill, Some("o/r"))];
    let global = [global("tdd", Kind::Skill, Some("o/r"))];
    let library = Catalog::from_items(vec![library_skill("o/r", "tdd")]);

    let rows = rows_of(&project, &global, &library);

    let held = items_in(&rows, Group::Global);
    assert_eq!(groups_of(&rows).len(), 2);
    assert_eq!(held[0].state, State::Held { in_project: true });
}

#[test]
fn a_global_only_item_can_be_installed_into_the_project_and_is_not_available_twice() {
    let global = [global("tdd", Kind::Skill, Some("o/r"))];
    let library = Catalog::from_items(vec![library_skill("o/r", "tdd")]);

    let rows = rows_of(&[], &global, &library);

    let held = items_in(&rows, Group::Global);
    assert!(matches!(held[0].state, State::Available(_)));
    assert!(items_in(&rows, Group::Available).is_empty());
}

#[test]
fn a_global_item_nothing_could_reinstall_is_held_and_not_in_the_project() {
    let global = [global("mine", Kind::Skill, None)];

    let rows = rows_of(&[], &global, &Catalog::default());

    let held = items_in(&rows, Group::Global);
    assert_eq!(held[0].state, State::Held { in_project: false });
}

#[test]
fn empty_groups_and_sections_are_left_out_and_sections_follow_the_display_order() {
    let project = [
        installed("h", Kind::Hook, None),
        installed("s", Kind::Skill, None),
        installed("m", Kind::Marketplace, None),
    ];

    let rows = rows_of(&project, &[], &Catalog::default());

    let kinds: Vec<ItemKind> = rows
        .iter()
        .filter_map(|row| match row {
            Row::Section { kind, .. } => Some(*kind),
            _ => None,
        })
        .collect();
    assert_eq!(groups_of(&rows), [(Group::Project, 2)]);
    assert_eq!(kinds, [ItemKind::Skill, ItemKind::Hook]);
}

#[test]
fn items_hang_under_their_section_and_sections_under_their_group() {
    let project = [
        installed("a", Kind::Skill, None),
        installed("b", Kind::Hook, None),
    ];

    let shape = shape(&rows_of(&project, &[], &Catalog::default()));

    let parents: Vec<Option<usize>> = shape.iter().map(|(p, _)| *p).collect();
    assert_eq!(parents, [None, Some(0), Some(1), Some(0), Some(3)]);
}

#[test]
fn the_same_section_in_two_groups_has_two_different_labels() {
    let project = [installed("a", Kind::Skill, None)];
    let global = [global("b", Kind::Skill, None)];

    let shape = shape(&rows_of(&project, &global, &Catalog::default()));

    let sections: Vec<&String> = shape
        .iter()
        .filter(|(parent, _)| parent.is_some_and(|p| p % 3 == 0))
        .map(|(_, label)| label)
        .collect();
    assert_ne!(sections[0], sections[1]);
}
