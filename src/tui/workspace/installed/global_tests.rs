use ratatui::crossterm::event::KeyCode;

use super::rows::tests::{global, installed};
use super::tests::{on_item, press};
use super::*;
use crate::tui::widgets::testing::render_to_buffer;
use skillz_harness::model::Kind;

/// `tdd` installed in the project and `grill` globally.
fn model() -> Model {
    Model {
        dir: "~/app".into(),
        installed: vec![installed("tdd", Kind::Skill, Some("o/r"))],
        global: vec![global("grill", Kind::Skill, Some("o/r"))],
        ..Model::default()
    }
}

/// The rows of a scope's one skill: its section, the package, the skill.
const SKILL: usize = 2;

fn loaded(scope: Scope) -> InstalledTab {
    let mut tab = InstalledTab::new(scope);
    tab.load(&model());
    tab.outline.open_all();
    tab
}

fn labels(tab: &InstalledTab) -> Vec<String> {
    tab.rows
        .iter()
        .filter_map(|row| match row {
            Row::Item(item) => Some(item.label.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_project_tab_also_lists_global_skills_for_inspection() {
    assert_eq!(labels(&loaded(Scope::Project)), ["tdd", "grill"]);
    assert_eq!(labels(&loaded(Scope::Global)), ["grill"]);
}

#[test]
fn the_global_tab_has_no_profile_keys() {
    let mut tab = loaded(Scope::Global);

    let compare = press(&mut tab, KeyCode::Char('p'));
    let apply = press(&mut tab, KeyCode::Char('a'));
    let save = press(&mut tab, KeyCode::Char('s'));

    assert_eq!(
        (compare, apply, save, tab.captures_keys()),
        (None, None, None, false)
    );
}

#[test]
fn removing_and_updating_on_the_global_tab_name_the_global_scope() {
    let mut tab = loaded(Scope::Global);
    on_item(&mut tab, SKILL);

    let remove = press(&mut tab, KeyCode::Char('r'));
    let update = press(&mut tab, KeyCode::Char('u'));

    assert_eq!(
        remove,
        Some(Act::Remove {
            scope: Scope::Global,
            ids: vec!["global-skill:grill".into()]
        })
    );
    assert_eq!(
        update,
        Some(Act::Update {
            scope: Scope::Global,
            ids: vec!["global-skill:grill".into()]
        })
    );
}

fn drawn_checkboxes(scope: Scope, width: u16, height: u16) -> usize {
    let mut tab = loaded(scope);
    let theme = Theme::dark();
    let buffer = render_to_buffer(width, height, |f| tab.draw(f, f.area(), &theme));
    buffer
        .content()
        .iter()
        .filter(|cell| cell.symbol() == theme.glyphs.checked)
        .count()
}

#[test]
fn both_tabs_draw_their_rows_at_small_and_large_sizes() {
    for scope in [Scope::Project, Scope::Global] {
        let count = if scope == Scope::Project { 2 } else { 1 };
        assert_eq!(drawn_checkboxes(scope, 80, 24), count, "{scope:?}");
        assert_eq!(drawn_checkboxes(scope, 50, 15), count, "{scope:?}");
    }
}

#[test]
fn marketplaces_can_be_removed_and_updated_from_either_scope() {
    for scope in [Scope::Project, Scope::Global] {
        let mut marketplace = installed("mp", Kind::Marketplace, Some("o/mp"));
        marketplace.scope = scope;
        marketplace.installer = skillz_harness::model::Installer::ClaudeCli;
        let id = marketplace.id.clone();
        let model = match scope {
            Scope::Project => Model {
                installed: vec![marketplace],
                ..Model::default()
            },
            Scope::Global => Model {
                global: vec![marketplace],
                ..Model::default()
            },
        };
        let mut tab = InstalledTab::new(scope);
        tab.load_open(&model);
        let at = tab
            .rows
            .iter()
            .position(|row| matches!(row, Row::Item(item) if item.id == id))
            .unwrap();
        on_item(&mut tab, at);
        assert_eq!(
            press(&mut tab, KeyCode::Char('r')),
            Some(Act::Remove {
                scope,
                ids: vec![id.clone()]
            })
        );
        assert_eq!(
            press(&mut tab, KeyCode::Char('u')),
            Some(Act::Update {
                scope,
                ids: vec![id]
            })
        );
    }
}

#[test]
fn inherited_items_cannot_be_picked_removed_updated_or_cleared() {
    let mut tab = loaded(Scope::Project);
    let inherited = tab
        .rows
        .iter()
        .position(|row| matches!(row, Row::Item(item) if item.id == "global-skill:grill"))
        .unwrap();
    on_item(&mut tab, inherited);

    assert_eq!(press(&mut tab, KeyCode::Char(' ')), None);
    assert!(tab.pending.is_empty());
    assert_eq!(press(&mut tab, KeyCode::Char('r')), None);
    assert_eq!(press(&mut tab, KeyCode::Char('u')), None);
    assert_eq!(
        press(&mut tab, KeyCode::Char('c')),
        Some(Act::Remove {
            scope: Scope::Project,
            ids: vec!["skill:tdd".into()],
        })
    );
}

#[test]
fn inherited_installs_draw_dimmed_while_project_installs_do_not() {
    use ratatui::style::Modifier;
    let mut tab = loaded(Scope::Project);
    let theme = Theme::dark();
    let buffer = render_to_buffer(100, 30, |frame| tab.draw(frame, frame.area(), &theme));
    let dimmed: Vec<_> = buffer
        .content()
        .iter()
        .filter(|cell| cell.symbol() == theme.glyphs.checked)
        .map(|cell| cell.modifier.contains(Modifier::DIM))
        .collect();
    assert_eq!(dimmed, [false, true]);
}

#[test]
fn inherited_plugins_expand_one_level_at_a_time_without_becoming_actionable() {
    let mut plugin = super::rows::tests::plugin_providing_a_skill_and_a_hook("global-plugin");
    plugin.scope = Scope::Global;
    let mut tab = InstalledTab::project();
    tab.load(&Model {
        global: vec![plugin],
        ..Model::default()
    });
    assert_eq!(tab.outline.visible(), [0]);
    press(&mut tab, KeyCode::Right);
    assert_eq!(tab.outline.visible(), [0, 1]);
    tab.outline.select_row(1);
    press(&mut tab, KeyCode::Right);
    assert_eq!(tab.outline.visible(), [0, 1, 2]);
    tab.outline.select_row(2);
    press(&mut tab, KeyCode::Enter);
    assert_eq!(tab.outline.visible(), [0, 1, 2, 3, 4]);
    assert!(tab.pending.is_empty());
    assert_eq!(press(&mut tab, KeyCode::Char('r')), None);
    tab.outline.select_row(3);
    assert_eq!(press(&mut tab, KeyCode::Char('r')), None);
    assert_eq!(press(&mut tab, KeyCode::Char('u')), None);
    assert_eq!(press(&mut tab, KeyCode::Char('c')), None);
}

#[test]
fn picking_a_project_item_marks_no_inherited_item_that_shares_its_id() {
    let mut tab = InstalledTab::project();
    tab.load(&Model {
        installed: vec![installed("tdd", Kind::Skill, Some("o/r"))],
        global: vec![Installed {
            id: "skill:tdd".into(),
            ..global("tdd", Kind::Skill, Some("o/r"))
        }],
        ..Model::default()
    });
    tab.outline.open_all();
    let project = tab
        .rows
        .iter()
        .position(|row| matches!(row, Row::Item(item) if item.id == "skill:tdd"))
        .unwrap();
    on_item(&mut tab, project);
    press(&mut tab, KeyCode::Char(' '));

    let theme = Theme::dark();
    let buffer = render_to_buffer(100, 30, |frame| tab.draw(frame, frame.area(), &theme));
    let marked = |glyph: &str| {
        buffer
            .content()
            .iter()
            .filter(|cell| cell.symbol() == glyph)
            .count()
    };

    assert_eq!(
        (marked(theme.glyphs.to_remove), marked(theme.glyphs.checked)),
        (1, 1)
    );
}
