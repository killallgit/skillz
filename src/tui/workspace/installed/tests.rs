use ratatui::crossterm::event::KeyCode;

use super::rows::tests::{installed, plugin_providing_a_skill_and_a_hook};
use super::*;
use crate::library::ItemKind;
use crate::model::Kind;
use crate::profile::Entry;
use crate::tui::widgets::testing::{key, keys, render_to_buffer};

pub(super) fn model() -> Model {
    let mut web = Profile::new("web");
    web.entries = vec![Entry::Skill {
        source: "o/r".into(),
        names: vec!["grill".into()],
        agents: vec![],
    }];
    Model {
        dir: "~/app".into(),
        installed: vec![
            plugin_providing_a_skill_and_a_hook("p@mp"),
            installed("tdd", Kind::Skill, Some("o/r")),
        ],
        global: vec![],
        library: Default::default(),
        profiles: vec![web],
        notice: None,
    }
}

pub(super) fn loaded() -> InstalledTab {
    let mut tab = InstalledTab::project();
    tab.load(&model());
    tab.outline.open_all();
    tab
}

pub(super) fn press(tab: &mut InstalledTab, code: KeyCode) -> Option<Act> {
    tab.handle(key(code))
}

pub(super) fn space() -> KeyCode {
    KeyCode::Char(' ')
}

/// The rows of `model()`: the plugins section, the plugin, the skills section, the `o/r`
/// package and its skill.
pub(super) const PLUGINS: usize = 0;
pub(super) const PLUGIN: usize = 1;
pub(super) const PACKAGE: usize = 3;
pub(super) const TDD: usize = 4;

pub(super) fn on_item(tab: &mut InstalledTab, row: usize) {
    tab.outline.select_row(row);
}

#[test]
fn enter_on_a_section_closes_and_reopens_it_without_asking_for_anything() {
    let mut tab = loaded();
    on_item(&mut tab, PLUGINS);

    let closed = (press(&mut tab, KeyCode::Enter), tab.outline.visible().len());
    let opened = (press(&mut tab, KeyCode::Enter), tab.outline.visible().len());

    assert_eq!((closed, opened), ((None, 4), (None, 5)));
}

#[test]
fn a_lists_the_profiles_and_the_chosen_one_replaces_everything() {
    let mut tab = loaded();

    let opened = press(&mut tab, KeyCode::Char('a'));
    let capturing = tab.captures_keys();
    let chosen = press(&mut tab, KeyCode::Enter);

    assert_eq!(opened, None);
    assert!(capturing && !tab.captures_keys());
    assert_eq!(
        chosen,
        Some(Act::ApplyExact {
            profile: "web".into()
        })
    );
}

#[test]
fn a_with_no_profiles_opens_nothing() {
    let mut tab = InstalledTab::project();
    tab.load(&Model::default());

    press(&mut tab, KeyCode::Char('a'));

    assert!(!tab.captures_keys());
}

#[test]
fn s_names_a_profile_saved_from_what_the_project_has() {
    let mut tab = loaded();

    press(&mut tab, KeyCode::Char('s'));
    type_text(&mut tab, "snap");
    let act = press(&mut tab, KeyCode::Enter);

    assert_eq!(
        act,
        Some(Act::SaveCurrent {
            name: "snap".into()
        })
    );
}

#[test]
fn the_picker_takes_every_key_until_it_closes() {
    let mut tab = loaded();

    press(&mut tab, KeyCode::Char('p'));
    let open = tab.captures_keys();
    press(&mut tab, KeyCode::Esc);

    assert!(open);
    assert!(!tab.captures_keys());
}

#[test]
fn the_filter_prompt_is_offered_while_typing_and_narrows_the_rows() {
    let mut tab = loaded();

    press(&mut tab, KeyCode::Char('/'));
    keys("tdd").into_iter().for_each(|k| {
        tab.handle(k);
    });
    let typing = tab.prompt().is_some();
    press(&mut tab, KeyCode::Enter);

    assert!(typing && tab.prompt().is_none());
    assert_eq!(tab.outline.visible(), [2, PACKAGE, TDD]);
}

#[test]
fn a_new_snapshot_keeps_the_closed_sections() {
    let mut tab = loaded();
    on_item(&mut tab, PLUGINS);
    press(&mut tab, KeyCode::Enter);

    tab.load(&model());

    assert_eq!(tab.outline.visible().len(), 4);
}

#[test]
fn what_a_plugin_provides_is_not_listed_among_the_skills_or_hooks() {
    let tab = loaded();

    let skills: Vec<&str> = tab
        .rows
        .iter()
        .filter_map(|row| match row {
            Row::Item(item) if item.kind == ItemKind::Skill => Some(item.label.as_str()),
            _ => None,
        })
        .collect();

    assert_eq!(skills, ["tdd"]);
}

#[test]
fn update_and_space_on_a_section_pick_and_remove_nothing() {
    let mut tab = loaded();
    on_item(&mut tab, PLUGINS);

    let acts: Vec<Option<Act>> = "u"
        .chars()
        .map(KeyCode::Char)
        .map(|code| press(&mut tab, code))
        .collect();
    press(&mut tab, space());

    assert!(acts.iter().all(Option::is_none));
    assert!(tab.pending.is_empty());
}

fn glyph_counts(tab: &mut InstalledTab, width: u16, height: u16) -> (usize, usize) {
    let theme = Theme::dark();
    let buffer = render_to_buffer(width, height, |f| tab.draw(f, f.area(), &theme));
    let count = |glyph: &str| {
        buffer
            .content()
            .iter()
            .filter(|cell| cell.symbol() == glyph)
            .count()
    };
    (count(theme.glyphs.checked), count(theme.glyphs.to_remove))
}

#[test]
fn every_installed_row_draws_checked_at_small_and_large_sizes() {
    for (width, height) in [(80, 24), (50, 12)] {
        let mut tab = loaded();

        assert_eq!(glyph_counts(&mut tab, width, height), (2, 0));
    }
}

fn type_text(tab: &mut InstalledTab, text: &str) {
    text.chars().for_each(|c| {
        press(tab, KeyCode::Char(c));
    });
}

#[test]
fn a_typed_source_is_handed_over_with_the_tabs_scope() {
    let mut tab = loaded();

    press(&mut tab, KeyCode::Char('A'));
    assert!(tab.captures_keys());
    type_text(&mut tab, "o/r");
    let act = press(&mut tab, KeyCode::Enter);

    assert_eq!(
        act,
        Some(Act::InstallSource {
            scope: Scope::Project,
            source: "o/r".into()
        })
    );
    assert!(!tab.captures_keys());
}

#[test]
fn escape_or_an_empty_source_installs_nothing_and_closes_the_prompt() {
    let mut tab = loaded();

    press(&mut tab, KeyCode::Char('A'));
    let empty = press(&mut tab, KeyCode::Enter);
    press(&mut tab, KeyCode::Char('A'));
    type_text(&mut tab, "o/r");
    let escaped = press(&mut tab, KeyCode::Esc);

    assert_eq!((empty, escaped), (None, None));
    assert!(!tab.captures_keys());
}
