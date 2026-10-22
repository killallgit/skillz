use ratatui::crossterm::event::KeyCode;

use super::rows::tests::{global, installed};
use super::tests::{on_item, press};
use super::*;
use crate::model::Kind;
use crate::tui::widgets::testing::render_to_buffer;

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
fn each_tab_lists_what_is_installed_in_its_own_scope_only() {
    assert_eq!(labels(&loaded(Scope::Project)), ["tdd"]);
    assert_eq!(labels(&loaded(Scope::Global)), ["grill"]);
}

#[test]
fn the_global_tab_has_no_profile_keys() {
    let mut tab = loaded(Scope::Global);

    let compare = press(&mut tab, KeyCode::Char('p'));
    let apply = press(&mut tab, KeyCode::Char('a'));

    assert_eq!((compare, apply, tab.captures_keys()), (None, None, false));
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
        assert_eq!(drawn_checkboxes(scope, 80, 24), 1, "{scope:?}");
        assert_eq!(drawn_checkboxes(scope, 50, 15), 1, "{scope:?}");
    }
}
