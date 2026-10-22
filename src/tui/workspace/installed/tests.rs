use ratatui::crossterm::event::KeyCode;

use super::rows::tests::{installed, library_skill};
use super::*;
use crate::library::Catalog;
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
        installed: vec![installed("tdd", Kind::Skill, Some("o/r"))],
        global: vec![],
        library: Catalog::from_items(vec![library_skill("o/r", "grill")]),
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

/// The row of the installed skill in `model()`; its section comes first.
pub(super) const TDD: usize = 1;
/// The row of the library skill in `model()`, after the `Available` header and its section.
pub(super) const GRILL: usize = 4;

pub(super) fn on_item(tab: &mut InstalledTab, row: usize) {
    tab.outline.select_row(row);
}

#[test]
fn enter_on_a_section_closes_and_reopens_it_without_asking_for_anything() {
    let mut tab = loaded();

    let closed = (press(&mut tab, KeyCode::Enter), tab.outline.visible().len());
    let opened = (press(&mut tab, KeyCode::Enter), tab.outline.visible().len());

    assert_eq!((closed, opened), ((None, 4), (None, 5)));
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
    assert_eq!(tab.outline.visible(), [2, 3, GRILL]);
}

#[test]
fn a_new_snapshot_keeps_the_cursor_and_the_closed_sections() {
    let mut tab = loaded();
    press(&mut tab, KeyCode::Enter);

    tab.load(&model());

    assert_eq!(tab.outline.visible().len(), 4);
}

#[test]
fn keys_on_a_hook_a_plugin_owns_do_nothing_and_pick_nothing() {
    let mut plugin = installed("p@mp", Kind::Plugin, Some("mp"));
    plugin.hooks = vec![crate::hook_text::HookEntry {
        event: "Stop".into(),
        matcher: None,
        command: "say done".into(),
    }];
    let mut tab = InstalledTab::project();
    tab.load_open(&Model {
        installed: vec![plugin],
        ..model()
    });
    let owned = tab
        .rows
        .iter()
        .position(|row| matches!(row, Row::Owned(_)))
        .unwrap();
    on_item(&mut tab, owned);
    let visible = tab.outline.visible().len();

    let acts: Vec<Option<Act>> = [space(), KeyCode::Enter]
        .into_iter()
        .chain("iru".chars().map(KeyCode::Char))
        .map(|code| press(&mut tab, code))
        .collect();

    assert!(acts.iter().all(Option::is_none));
    assert!(tab.pending.is_empty());
    assert_eq!(tab.outline.visible().len(), visible);
}

#[test]
fn enter_on_a_plugin_folds_the_skills_it_provides() {
    let mut plugin = installed("p@mp", Kind::Plugin, Some("mp"));
    plugin.children = vec![format!("{}tdd", crate::model::SKILL_CHILD)];
    let mut tab = InstalledTab::project();
    tab.load_open(&Model {
        installed: vec![plugin],
        ..Model::default()
    });
    let open = tab.outline.visible().len();
    on_item(&mut tab, 1);

    press(&mut tab, KeyCode::Enter);

    assert_eq!((open, tab.outline.visible().len()), (3, 2));
}

fn checkboxes(tab: &mut InstalledTab, width: u16, height: u16) -> (usize, usize) {
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
