use ratatui::crossterm::event::KeyCode;

use super::rows::tests::{global, installed, library_skill};
use super::tests::{GRILL, TDD, loaded, on_item, press, space};
use super::*;
use crate::library::Catalog;
use crate::model::{Kind, Scope};
use crate::profile::Entry;
use crate::tui::widgets::testing::keys;

/// A library skill that is installed globally too, and one that is not.
const GLOBAL_GRILL: usize = 2;
const ZOOM: usize = 3;

fn with_global_grill() -> InstalledTab {
    let mut tab = InstalledTab::project();
    tab.load_open(&Model {
        installed: vec![installed("tdd", Kind::Skill, Some("o/r"))],
        global: vec![global("grill", Kind::Skill, Some("o/r"))],
        library: Catalog::from_items(vec![
            library_skill("o/r", "grill"),
            library_skill("o/r", "zoom"),
        ]),
        ..Model::default()
    });
    tab
}

fn entry(name: &str) -> Entry {
    library_skill("o/r", name).entry
}

fn change(install: Vec<Entry>, remove: Vec<&str>) -> Option<Act> {
    let remove = remove.into_iter().map(String::from).collect();
    Some(Act::Change {
        scope: Scope::Project,
        install,
        remove,
    })
}

fn answer(tab: &mut InstalledTab, code: KeyCode) -> Option<Act> {
    press(tab, code)
}

#[test]
fn i_installs_the_highlighted_library_item_at_once() {
    let mut tab = loaded();
    on_item(&mut tab, GRILL);

    let act = press(&mut tab, KeyCode::Char('i'));

    assert_eq!(act, change(vec![entry("grill")], vec![]));
    assert!(!tab.captures_keys());
}

#[test]
fn r_removes_the_highlighted_project_item_at_once() {
    let mut tab = loaded();
    on_item(&mut tab, TDD);

    let act = press(&mut tab, KeyCode::Char('r'));

    assert_eq!(act, change(vec![], vec!["skill:tdd"]));
}

#[test]
fn r_on_a_package_removes_everything_installed_from_it() {
    let mut tab = InstalledTab::project();
    tab.load_open(&Model {
        installed: vec![
            installed("tdd", Kind::Skill, Some("o/r")),
            installed("grill", Kind::Skill, Some("o/r")),
            installed("other", Kind::Skill, Some("x/y")),
        ],
        ..Model::default()
    });
    on_item(&mut tab, 0);

    let act = press(&mut tab, KeyCode::Char('r'));

    assert_eq!(act, change(vec![], vec!["skill:grill", "skill:tdd"]));
}

#[test]
fn i_on_an_available_package_installs_everything_it_offers() {
    let mut tab = InstalledTab::project();
    tab.load_open(&Model {
        library: Catalog::from_items(vec![
            library_skill("o/r", "grill"),
            library_skill("o/r", "zoom"),
        ]),
        ..Model::default()
    });
    on_item(&mut tab, 0);

    let act = press(&mut tab, KeyCode::Char('i'));

    assert_eq!(act, change(vec![entry("grill"), entry("zoom")], vec![]));
}

#[test]
fn i_and_r_do_nothing_where_they_do_not_apply() {
    let mut tab = loaded();
    let cases = [(TDD, 'i'), (GRILL, 'r')];

    let acts: Vec<Option<Act>> = cases
        .into_iter()
        .map(|(row, key)| {
            on_item(&mut tab, row);
            press(&mut tab, KeyCode::Char(key))
        })
        .collect();

    assert!(acts.iter().all(Option::is_none));
    assert!(!tab.captures_keys());
}

#[test]
fn i_on_a_global_item_asks_first_and_yes_continues_to_the_plan() {
    let mut tab = with_global_grill();
    on_item(&mut tab, GLOBAL_GRILL);

    let asked = press(&mut tab, KeyCode::Char('i'));
    let waiting = tab.captures_keys();
    let act = answer(&mut tab, KeyCode::Char('y'));

    assert_eq!((asked, waiting), (None, true));
    assert_eq!(act, change(vec![entry("grill")], vec![]));
    assert!(!tab.captures_keys());
}

#[test]
fn the_duplicate_question_defaults_to_no() {
    let mut tab = with_global_grill();
    on_item(&mut tab, GLOBAL_GRILL);
    press(&mut tab, KeyCode::Char('i'));

    let act = answer(&mut tab, KeyCode::Enter);

    assert_eq!(act, None);
    assert!(!tab.captures_keys());
}

#[test]
fn a_batch_asks_once_and_no_drops_only_the_global_items() {
    let mut tab = with_global_grill();
    for row in [GLOBAL_GRILL, ZOOM] {
        on_item(&mut tab, row);
        press(&mut tab, space());
    }

    let asked = press(&mut tab, KeyCode::Enter);
    let act = answer(&mut tab, KeyCode::Char('n'));

    assert_eq!(asked, None);
    assert_eq!(act, change(vec![entry("zoom")], vec![]));
}

#[test]
fn a_batch_answered_yes_keeps_every_item() {
    let mut tab = with_global_grill();
    for row in [GLOBAL_GRILL, ZOOM] {
        on_item(&mut tab, row);
        press(&mut tab, space());
    }
    press(&mut tab, KeyCode::Enter);

    let act = answer(&mut tab, KeyCode::Char('y'));

    assert_eq!(act, change(vec![entry("grill"), entry("zoom")], vec![]));
}

#[test]
fn a_batch_without_global_items_goes_straight_to_the_plan() {
    let mut tab = with_global_grill();
    on_item(&mut tab, ZOOM);
    press(&mut tab, space());

    let act = press(&mut tab, KeyCode::Enter);

    assert_eq!(act, change(vec![entry("zoom")], vec![]));
}

#[test]
fn typing_i_or_r_into_the_filter_is_text_not_a_command() {
    let mut tab = loaded();
    press(&mut tab, KeyCode::Char('/'));

    let acts: Vec<Option<Act>> = keys("ir").into_iter().map(|k| tab.handle(k)).collect();

    assert!(acts.iter().all(Option::is_none));
    assert_eq!(tab.outline.query(), "ir");
}
