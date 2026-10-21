use ratatui::crossterm::event::KeyCode;

use super::rows::tests::{hook, skill};
use super::*;
use crate::library::Catalog;
use crate::tui::widgets::testing::{key, render_to_buffer, row_text};

fn loaded() -> LibraryTab {
    let mut tab = LibraryTab::new();
    let library = Catalog::from_items(vec![skill("o/r", "tdd"), hook("notify")]);
    tab.load(&Model {
        library,
        ..Model::default()
    });
    tab
}

fn press(tab: &mut LibraryTab, code: KeyCode) -> Option<Act> {
    tab.handle(key(code))
}

fn type_text(tab: &mut LibraryTab, text: &str) {
    text.chars().for_each(|c| {
        press(tab, KeyCode::Char(c));
    });
}

/// Sections are Skills(0), tdd(1), Hooks(2), notify(3).
fn on_row(tab: &mut LibraryTab, row: usize) {
    (0..row).for_each(|_| {
        press(tab, KeyCode::Down);
    });
}

#[test]
fn a_typed_line_is_handed_over_as_typed_when_submitted() {
    let mut tab = loaded();

    press(&mut tab, KeyCode::Char('a'));
    type_text(&mut tab, "skill o/r grill");
    let act = press(&mut tab, KeyCode::Enter);

    assert_eq!(
        act,
        Some(Act::LibraryAdd {
            spec: "skill o/r grill".into()
        })
    );
    assert!(!tab.captures_keys());
}

#[test]
fn escape_or_an_empty_line_adds_nothing_and_closes_the_prompt() {
    let mut tab = loaded();

    press(&mut tab, KeyCode::Char('a'));
    let empty = press(&mut tab, KeyCode::Enter);
    press(&mut tab, KeyCode::Char('a'));
    type_text(&mut tab, "skill");
    let cancelled = press(&mut tab, KeyCode::Esc);

    assert_eq!((empty, cancelled), (None, None));
    assert!(!tab.captures_keys());
}

#[test]
fn the_prompt_takes_every_key_so_letters_are_text() {
    let mut tab = loaded();

    press(&mut tab, KeyCode::Char('a'));
    let act = press(&mut tab, KeyCode::Char('i'));

    assert_eq!(act, None);
    assert!(tab.captures_keys());
}

#[test]
fn i_asks_to_import_what_is_installed() {
    let mut tab = loaded();

    let act = press(&mut tab, KeyCode::Char('i'));

    assert_eq!(act, Some(Act::LibraryImport));
}

#[test]
fn removing_an_item_needs_a_yes() {
    let mut tab = loaded();
    on_row(&mut tab, 1);

    press(&mut tab, KeyCode::Char('x'));
    let asked = tab.captures_keys();
    let act = press(&mut tab, KeyCode::Char('y'));

    assert!(asked);
    assert_eq!(act, Some(Act::LibraryRemove(skill("o/r", "tdd").entry)));
}

#[test]
fn declining_or_removing_a_section_removes_nothing() {
    let mut tab = loaded();

    press(&mut tab, KeyCode::Char('x'));
    let on_section = tab.captures_keys();
    on_row(&mut tab, 1);
    press(&mut tab, KeyCode::Char('x'));
    let declined = press(&mut tab, KeyCode::Char('n'));

    assert!(!on_section);
    assert_eq!(declined, None);
    assert!(!tab.captures_keys());
}

#[test]
fn space_on_a_section_folds_it_and_hides_its_items() {
    let mut tab = loaded();
    let before = tab.outline.visible().len();

    press(&mut tab, KeyCode::Char(' '));

    assert_eq!((before, tab.outline.visible().len()), (4, 3));
}

#[test]
fn a_filter_narrows_the_rows_to_what_matches() {
    let mut tab = loaded();

    press(&mut tab, KeyCode::Char('/'));
    type_text(&mut tab, "notify");
    press(&mut tab, KeyCode::Enter);

    assert!(tab.outline.is_filtered());
    assert_eq!(tab.outline.visible(), [2, 3]);
}

#[test]
fn the_screen_draws_items_under_their_sections() {
    let mut tab = loaded();
    let theme = Theme::dark();

    let buffer = render_to_buffer(60, 12, |frame| tab.draw(frame, frame.area(), &theme));

    let text: String = (0..12).map(|y| row_text(&buffer, y)).collect();
    assert!(text.contains("tdd") && text.contains("notify"));
}
