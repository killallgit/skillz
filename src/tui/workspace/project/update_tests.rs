use ratatui::crossterm::event::KeyCode;

use super::tests::{loaded, on_item, press};
use super::*;

fn u() -> KeyCode {
    KeyCode::Char('u')
}

#[test]
fn u_on_an_installed_item_asks_to_update_it() {
    let mut tab = loaded();
    on_item(&mut tab, 1);

    let act = press(&mut tab, u());

    assert_eq!(
        act,
        Some(Act::Update {
            ids: vec!["skill:tdd".into()]
        })
    );
}

#[test]
fn u_on_a_section_asks_to_update_every_installed_item_in_it() {
    let mut tab = loaded();

    let act = press(&mut tab, u());

    assert_eq!(
        act,
        Some(Act::Update {
            ids: vec!["skill:tdd".into()]
        })
    );
}

#[test]
fn u_on_a_library_item_that_is_not_installed_does_nothing() {
    let mut tab = loaded();
    on_item(&mut tab, 2);

    let act = press(&mut tab, u());

    assert_eq!(act, None);
}

#[test]
fn u_on_an_item_without_an_update_does_nothing() {
    let mut model = super::tests::model();
    model.installed[0].installer = crate::model::Installer::Local;
    let mut tab = ProjectTab::new();
    tab.load(&model);
    on_item(&mut tab, 1);

    let act = press(&mut tab, u());

    assert_eq!(act, None);
}

#[test]
fn u_keeps_the_picks_for_installing_and_removing_separate() {
    let mut tab = loaded();
    on_item(&mut tab, 2);
    press(&mut tab, KeyCode::Char(' '));

    press(&mut tab, u());

    assert!(!tab.pending.is_empty());
}

#[test]
fn an_outdated_item_is_marked_bold_with_a_star() {
    let mut tab = loaded();
    tab.mark_outdated(&["skill:tdd".to_string()].into());

    let theme = crate::tui::theme::Theme::dark();
    let marked =
        crate::tui::widgets::testing::render_to_buffer(80, 12, |f| tab.draw(f, f.area(), &theme));

    let star = marked
        .content()
        .iter()
        .any(|cell| cell.symbol() == "*" && cell.modifier.contains(ratatui::style::Modifier::BOLD));
    assert!(star);
}
