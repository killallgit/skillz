use ratatui::crossterm::event::KeyCode;

use super::rows::tests::{installed, library_skill};
use super::*;
use crate::library::Catalog;
use crate::model::Kind;
use crate::profile::Entry;
use crate::tui::widgets::testing::{key, keys, render_to_buffer};

fn model() -> Model {
    let mut web = Profile::new("web");
    web.entries = vec![Entry::Skill {
        source: "o/r".into(),
        names: vec!["grill".into()],
        agents: vec![],
    }];
    Model {
        dir: "~/app".into(),
        installed: vec![installed("tdd", Kind::Skill, Some("o/r"))],
        library: Catalog::from_items(vec![library_skill("o/r", "grill")]),
        profiles: vec![web],
        notice: None,
    }
}

fn loaded() -> ProjectTab {
    let mut tab = ProjectTab::new();
    tab.load(&model());
    tab
}

fn press(tab: &mut ProjectTab, code: KeyCode) -> Option<Act> {
    tab.handle(key(code))
}

fn space() -> KeyCode {
    KeyCode::Char(' ')
}

/// Section, installed tdd, available grill; the cursor starts on the section.
fn on_item(tab: &mut ProjectTab, row: usize) {
    (0..row).for_each(|_| {
        press(tab, KeyCode::Down);
    });
}

#[test]
fn space_on_an_installed_item_asks_to_remove_it_by_id() {
    let mut tab = loaded();
    on_item(&mut tab, 1);

    let act = press(&mut tab, space());

    assert_eq!(
        act,
        Some(Act::Remove {
            id: "skill:tdd".into()
        })
    );
}

#[test]
fn space_on_a_library_item_asks_to_install_its_entry() {
    let mut tab = loaded();
    on_item(&mut tab, 2);

    let act = press(&mut tab, space());

    assert_eq!(act, Some(Act::Install(library_skill("o/r", "grill").entry)));
}

#[test]
fn enter_on_a_section_closes_and_reopens_it_without_asking_for_anything() {
    let mut tab = loaded();

    let closed = (press(&mut tab, KeyCode::Enter), tab.outline.visible().len());
    let opened = (press(&mut tab, KeyCode::Enter), tab.outline.visible().len());

    assert_eq!((closed, opened), ((None, 1), (None, 3)));
}

#[test]
fn applying_needs_a_compared_profile() {
    let mut tab = loaded();
    let without = press(&mut tab, KeyCode::Char('a'));

    press(&mut tab, KeyCode::Char('p'));
    press(&mut tab, KeyCode::Down);
    press(&mut tab, KeyCode::Enter);
    let with = press(&mut tab, KeyCode::Char('a'));

    assert_eq!(without, None);
    assert_eq!(
        with,
        Some(Act::Apply {
            profile: "web".into()
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
    keys("grill").into_iter().for_each(|k| {
        tab.handle(k);
    });
    let typing = tab.prompt().is_some();
    press(&mut tab, KeyCode::Enter);

    assert!(typing && tab.prompt().is_none());
    assert_eq!(tab.outline.visible(), [0, 2]);
}

#[test]
fn a_new_snapshot_keeps_the_cursor_and_the_closed_sections() {
    let mut tab = loaded();
    press(&mut tab, KeyCode::Enter);

    tab.load(&model());

    assert_eq!(tab.outline.visible().len(), 1);
}

fn checkboxes(tab: &mut ProjectTab, width: u16, height: u16) -> (usize, usize) {
    let theme = Theme::dark();
    let buffer = render_to_buffer(width, height, |f| tab.draw(f, f.area(), &theme));
    let glyphs = theme.glyphs;
    let count = |glyph: &str| {
        buffer
            .content()
            .iter()
            .filter(|cell| cell.symbol() == glyph)
            .count()
    };
    (count(glyphs.checked), count(glyphs.unchecked))
}

#[test]
fn installed_items_draw_checked_and_library_items_unchecked() {
    let mut tab = loaded();

    assert_eq!(checkboxes(&mut tab, 80, 24), (1, 1));
}

#[test]
fn a_small_terminal_still_draws_the_rows() {
    let mut tab = loaded();

    assert_eq!(checkboxes(&mut tab, 50, 12), (1, 1));
}
