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

fn sections_of(rows: &[Row], wanted: Part) -> Vec<ItemKind> {
    rows.iter()
        .filter_map(|row| match row {
            Row::Section { part, kind, .. } if *part == wanted => Some(*kind),
            _ => None,
        })
        .collect()
}

#[test]
fn installed_sections_come_first_then_one_available_header_with_its_own_sections() {
    let here = [installed("tdd", Kind::Skill, Some("o/r"))];
    let library = Catalog::from_items(vec![library_skill("o/r", "zoom")]);

    let rows = rows_of(&here, &[], &library);

    let header = rows.iter().position(|r| matches!(r, Row::Available { .. }));
    assert_eq!(header, Some(2));
    assert_eq!(rows.len(), 5);
    assert_eq!(sections_of(&rows, Part::Installed), [ItemKind::Skill]);
    assert_eq!(sections_of(&rows, Part::Available), [ItemKind::Skill]);
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
fn empty_parts_and_sections_are_left_out_and_sections_follow_the_display_order() {
    let here = [
        installed("h", Kind::Hook, None),
        installed("s", Kind::Skill, None),
        installed("m", Kind::Marketplace, None),
    ];

    let rows = rows_of(&here, &[], &Catalog::default());

    assert_eq!(
        sections_of(&rows, Part::Installed),
        [ItemKind::Skill, ItemKind::Hook]
    );
    assert!(sections_of(&rows, Part::Available).is_empty());
}

#[test]
fn items_hang_under_their_section_and_available_sections_under_their_header() {
    let here = [installed("a", Kind::Skill, None)];
    let library = Catalog::from_items(vec![library_skill("o/r", "zoom")]);

    let shape = shape(&rows_of(&here, &[], &library));

    let parents: Vec<Option<usize>> = shape.iter().map(|(p, _)| *p).collect();
    assert_eq!(parents, [None, Some(0), None, Some(2), Some(3)]);
}

#[test]
fn the_same_kind_installed_and_available_has_two_different_labels() {
    let here = [installed("a", Kind::Skill, None)];
    let library = Catalog::from_items(vec![library_skill("o/r", "zoom")]);

    let shape = shape(&rows_of(&here, &[], &library));

    assert_ne!(shape[0].1, shape[3].1);
}
