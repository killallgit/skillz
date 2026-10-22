use super::*;
use crate::model::{Agent, Installer, Kind, Scope};

pub fn installed(name: &str, kind: Kind, source: Option<&str>) -> Augmentation {
    Augmentation {
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

fn plugin_with_hook(name: &str) -> Augmentation {
    Augmentation {
        hooks: vec![crate::hook_text::HookEntry {
            event: "Stop".into(),
            matcher: None,
            command: "say done".into(),
        }],
        ..installed(name, Kind::Plugin, Some("mp"))
    }
}

#[test]
fn a_plugins_hooks_follow_it_as_owned_rows_not_in_the_hooks_section() {
    let here = [plugin_with_hook("p@mp")];

    let rows = rows_of(&here, &[], &Catalog::default());

    assert!(matches!(
        rows.as_slice(),
        [Row::Section { .. }, Row::Item(_), Row::Owned(_)]
    ));
    assert_eq!(sections_of(&rows, Part::Installed), [ItemKind::Plugin]);
}

#[test]
fn an_owned_row_hangs_under_its_plugin_and_says_which_plugin_adds_it() {
    let here = [plugin_with_hook("p@mp")];

    let rows = rows_of(&here, &[], &Catalog::default());

    let parents: Vec<Option<usize>> = shape(&rows).iter().map(|(p, _)| *p).collect();
    assert_eq!(parents, [None, Some(0), Some(1)]);
    let Row::Owned(owned) = &rows[2] else {
        panic!("not an owned row")
    };
    assert!(owned.detail.contains("p@mp"));
}

#[test]
fn unowned_hooks_are_titled_apart_from_available_ones() {
    let installed_title = title(Part::Installed, ItemKind::Hook);
    let available_title = title(Part::Available, ItemKind::Hook);

    assert_ne!(installed_title, available_title);
    assert_eq!(installed_title, crate::hooks::UNOWNED_HOOKS);
}

fn subagent_in(agent: Agent) -> Augmentation {
    Augmentation {
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
    assert!(matches!(rows[0], Row::Section { total: 1, .. }));
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
