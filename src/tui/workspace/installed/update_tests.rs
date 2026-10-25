use ratatui::crossterm::event::KeyCode;

use super::tests::{PACKAGE, TDD, loaded, on_item, press};
use super::*;
use skillz_harness::model::Scope;

fn u() -> KeyCode {
    KeyCode::Char('u')
}

#[test]
fn u_on_an_installed_item_asks_to_update_it() {
    let mut tab = loaded();
    on_item(&mut tab, TDD);

    let act = press(&mut tab, u());

    assert_eq!(
        act,
        Some(Act::Update {
            scope: Scope::Project,
            ids: vec!["skill:tdd".into()]
        })
    );
}

#[test]
fn u_on_a_package_updates_its_items_whether_expanded_or_collapsed() {
    for expanded in [true, false] {
        let mut tab = loaded();
        on_item(&mut tab, PACKAGE);
        if !expanded {
            press(&mut tab, KeyCode::Enter);
        }

        let act = press(&mut tab, u());

        // Assert package actions depend on its items, not its current presentation.
        assert_eq!(
            act,
            Some(Act::Update {
                scope: Scope::Project,
                ids: vec!["skill:tdd".into()],
            })
        );
    }
}

#[test]
fn u_on_an_item_without_an_update_does_nothing() {
    let mut model = super::tests::model();
    model.installed[1].installer = skillz_harness::model::Installer::Local;
    let mut tab = InstalledTab::project();
    tab.load(&model);
    on_item(&mut tab, TDD);

    let act = press(&mut tab, u());

    assert_eq!(act, None);
}

#[test]
fn u_keeps_the_picks() {
    let mut tab = loaded();
    on_item(&mut tab, TDD);
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

#[test]
fn updating_a_merged_package_selects_each_installed_copy() {
    use super::rows::tests::installed;
    use skillz_harness::model::{Agent, Kind};

    let copies: Vec<_> = [Agent::Claude, Agent::Codex]
        .into_iter()
        .map(|agent| {
            let mut item = installed("tdd", Kind::Skill, Some("o/r"));
            item.id = format!("skill:{agent}:tdd");
            item.agents = vec![agent];
            item
        })
        .collect();
    let expected = copies.iter().map(|item| item.id.clone()).collect();
    let model = Model {
        installed: crate::app::merge::merge(copies),
        ..Model::default()
    };
    let mut tab = InstalledTab::project();
    tab.load_open(&model);
    on_item(&mut tab, 1);

    let act = press(&mut tab, u());

    // Assert a collapsed aggregate package asks to update both physical copies.
    assert_eq!(
        act,
        Some(Act::Update {
            scope: Scope::Project,
            ids: expected
        })
    );
}
