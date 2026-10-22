use ratatui::crossterm::event::KeyCode;

use super::rows::tests::installed;
use super::tests::{PACKAGE, PLUGINS, TDD, loaded, model, on_item, press, space};
use super::*;
use crate::model::{Kind, Scope};
use crate::tui::widgets::testing::render_to_buffer;

#[test]
fn space_picks_an_item_and_again_unpicks_it() {
    let mut tab = loaded();
    on_item(&mut tab, TDD);

    press(&mut tab, space());
    let picked = tab.pending.picks();
    press(&mut tab, space());

    assert_eq!(picked, ["skill:tdd"]);
    assert!(tab.pending.is_empty());
}

#[test]
fn space_on_a_package_picks_everything_in_it_and_again_unpicks_it() {
    let mut tab = loaded();
    on_item(&mut tab, PACKAGE);

    press(&mut tab, space());
    let picked = tab.pending.picks();
    press(&mut tab, space());

    assert_eq!(picked, ["skill:tdd"]);
    assert!(tab.pending.is_empty());
    assert_eq!(tab.outline.visible().len(), 5);
}

#[test]
fn space_on_a_section_folds_it_and_picks_nothing() {
    let mut tab = loaded();
    on_item(&mut tab, PLUGINS);

    press(&mut tab, space());

    assert!(tab.pending.is_empty());
    assert_eq!(tab.outline.visible().len(), 4);
}

#[test]
fn enter_applies_the_picks_as_one_removal() {
    let mut tab = loaded();
    for row in [TDD, PLUGIN_ROW] {
        on_item(&mut tab, row);
        press(&mut tab, space());
    }

    let act = press(&mut tab, KeyCode::Enter);

    assert_eq!(
        act,
        Some(Act::Remove {
            scope: Scope::Project,
            ids: vec!["skill:tdd".into(), "plugin:p@mp".into()]
        })
    );
}

const PLUGIN_ROW: usize = super::tests::PLUGIN;

#[test]
fn escape_clears_the_picks_before_anything_else() {
    let mut tab = loaded();
    on_item(&mut tab, TDD);
    press(&mut tab, space());

    press(&mut tab, KeyCode::Esc);

    assert!(tab.pending.is_empty());
}

#[test]
fn a_pick_survives_a_new_snapshot_while_the_item_is_there() {
    let mut tab = loaded();
    on_item(&mut tab, TDD);
    press(&mut tab, space());

    tab.load_open(&model());

    assert!(!tab.pending.is_empty());
}

#[test]
fn a_pick_is_dropped_when_its_item_is_gone() {
    let mut tab = loaded();
    on_item(&mut tab, TDD);
    press(&mut tab, space());
    let mut without = model();
    without.installed.retain(|item| item.kind != Kind::Skill);
    without
        .installed
        .push(installed("other", Kind::Skill, Some("o/r")));

    tab.load_open(&without);

    assert!(tab.pending.is_empty());
}

#[test]
fn a_package_counts_what_is_picked_in_it() {
    let mut tab = loaded();
    on_item(&mut tab, TDD);
    press(&mut tab, space());
    let items = tab.rows.iter().filter_map(|row| match row {
        Row::Item(item) => Some(item),
        _ => None,
    });

    assert_eq!(tab.pending.count(items), 1);
}

fn picked_markers(glyphs: crate::tui::glyphs::Glyphs) -> usize {
    let mut tab = InstalledTab::project();
    tab.load_open(&Model {
        dir: "app".into(),
        ..model()
    });
    on_item(&mut tab, TDD);
    press(&mut tab, space());
    let mut theme = Theme::dark();
    theme.glyphs = glyphs;
    let buffer = render_to_buffer(80, 24, |f| tab.draw(f, f.area(), &theme));
    buffer
        .content()
        .iter()
        .filter(|c| c.symbol() == glyphs.to_remove)
        .count()
}

#[test]
fn a_picked_row_draws_its_own_marker_in_both_glyph_sets() {
    use crate::tui::glyphs::Glyphs;

    assert_eq!(picked_markers(Glyphs::UNICODE), 1);
    assert_eq!(picked_markers(Glyphs::ASCII), 1);
}
