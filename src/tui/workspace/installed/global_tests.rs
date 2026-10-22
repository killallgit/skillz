use ratatui::crossterm::event::KeyCode;

use super::rows::tests::{global, installed, library_skill};
use super::tests::{on_item, press};
use super::*;
use crate::library::Catalog;
use crate::model::Kind;
use crate::profile::Entry;
use crate::tui::widgets::testing::render_to_buffer;

/// The rows of the global skill and of the library skill that is not installed anywhere.
const GLOBAL_GRILL: usize = 1;
const ZOOM: usize = 4;

/// `tdd` installed in the project, `grill` globally, `zoom` only in the library.
fn model() -> Model {
    Model {
        dir: "~/app".into(),
        installed: vec![installed("tdd", Kind::Skill, Some("o/r"))],
        global: vec![global("grill", Kind::Skill, Some("o/r"))],
        library: Catalog::from_items(vec![
            library_skill("o/r", "grill"),
            library_skill("o/r", "zoom"),
        ]),
        ..Model::default()
    }
}

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

fn entry(name: &str) -> Entry {
    library_skill("o/r", name).entry
}

#[test]
fn each_tab_installs_what_is_installed_in_its_own_scope_only() {
    let project = loaded(Scope::Project);
    let global = loaded(Scope::Global);

    let in_project = labels(&project);
    let in_global = labels(&global);

    assert!(in_project[0].starts_with("tdd"));
    assert!(in_global[0].starts_with("grill"));
    assert!(!in_global.iter().any(|label| label.starts_with("tdd")));
}

#[test]
fn only_the_project_tab_tags_what_is_also_installed_globally() {
    let project = loaded(Scope::Project);
    let global = loaded(Scope::Global);
    let tagged = |tab: &InstalledTab| {
        tab.rows
            .iter()
            .any(|row| matches!(row, Row::Item(item) if item.global))
    };

    assert!(tagged(&project));
    assert!(!tagged(&global));
}

#[test]
fn the_global_tab_has_no_profile_keys() {
    let mut tab = loaded(Scope::Global);

    let compare = press(&mut tab, KeyCode::Char('p'));
    let apply = press(&mut tab, KeyCode::Char('a'));

    assert_eq!((compare, apply, tab.captures_keys()), (None, None, false));
}

#[test]
fn installing_removing_and_updating_on_the_global_tab_name_the_global_scope() {
    let mut tab = loaded(Scope::Global);

    on_item(&mut tab, GLOBAL_GRILL);
    let remove = press(&mut tab, KeyCode::Char('r'));
    let update = press(&mut tab, KeyCode::Char('u'));
    on_item(&mut tab, ZOOM);
    let install = press(&mut tab, KeyCode::Char('i'));

    assert_eq!(
        remove,
        Some(Act::Change {
            scope: Scope::Global,
            install: vec![],
            remove: vec!["global-skill:grill".into()]
        })
    );
    assert_eq!(
        update,
        Some(Act::Update {
            scope: Scope::Global,
            ids: vec!["global-skill:grill".into()]
        })
    );
    assert_eq!(
        install,
        Some(Act::Change {
            scope: Scope::Global,
            install: vec![entry("zoom")],
            remove: vec![]
        })
    );
}

#[test]
fn installing_globally_never_asks_about_a_global_duplicate() {
    let mut tab = loaded(Scope::Global);
    on_item(&mut tab, ZOOM);

    let act = press(&mut tab, KeyCode::Char('i'));

    assert!(act.is_some());
    assert!(!tab.captures_keys());
}

fn drawn_checkboxes(scope: Scope, width: u16, height: u16) -> (usize, usize) {
    let mut tab = loaded(scope);
    let theme = Theme::dark();
    let buffer = render_to_buffer(width, height, |f| tab.draw(f, f.area(), &theme));
    let count = |glyph: &str| {
        buffer
            .content()
            .iter()
            .filter(|cell| cell.symbol() == glyph)
            .count()
    };
    (count(theme.glyphs.checked), count(theme.glyphs.unchecked))
}

#[test]
fn both_tabs_draw_their_rows_at_small_and_large_sizes() {
    for (scope, expected) in [(Scope::Project, (1, 2)), (Scope::Global, (1, 1))] {
        assert_eq!(drawn_checkboxes(scope, 80, 24), expected, "{scope:?}");
        assert_eq!(drawn_checkboxes(scope, 50, 15), expected, "{scope:?}");
    }
}
