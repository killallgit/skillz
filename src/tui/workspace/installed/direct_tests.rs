use ratatui::crossterm::event::KeyCode;

use super::rows::tests::installed;
use super::tests::{PLUGIN, PLUGINS, TDD, loaded, on_item, press};
use super::*;
use crate::harness::model::{Kind, Scope};

fn remove(ids: &[&str]) -> Option<Act> {
    Some(Act::Remove {
        scope: Scope::Project,
        ids: ids.iter().map(|id| id.to_string()).collect(),
    })
}

#[test]
fn r_removes_the_highlighted_item_at_once() {
    let mut tab = loaded();
    on_item(&mut tab, TDD);

    assert_eq!(press(&mut tab, KeyCode::Char('r')), remove(&["skill:tdd"]));
}

#[test]
fn r_on_a_plugin_removes_the_whole_plugin() {
    let mut tab = loaded();
    on_item(&mut tab, PLUGIN);

    assert_eq!(
        press(&mut tab, KeyCode::Char('r')),
        remove(&["plugin:p@mp"])
    );
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
    let package = tab
        .rows
        .iter()
        .position(|row| matches!(row, Row::Package(_)))
        .unwrap();
    on_item(&mut tab, package);

    let act = press(&mut tab, KeyCode::Char('r'));

    assert_eq!(act, remove(&["skill:grill", "skill:tdd"]));
}

#[test]
fn r_on_a_section_marks_every_item_in_it_for_removal() {
    let mut tab = loaded();
    on_item(&mut tab, PLUGINS);

    let act = press(&mut tab, KeyCode::Char('r'));

    assert_eq!(act, None);
    assert_eq!(tab.pending.picks(), ["plugin:p@mp"]);
}

#[test]
fn typing_r_into_the_filter_is_text_not_a_command() {
    let mut tab = loaded();
    press(&mut tab, KeyCode::Char('/'));

    let act = press(&mut tab, KeyCode::Char('r'));

    assert_eq!(act, None);
    assert_eq!(tab.outline.query(), "r");
}
