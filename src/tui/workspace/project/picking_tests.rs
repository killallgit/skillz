use ratatui::crossterm::event::KeyCode;

use super::rows::tests::{installed, library_skill};
use super::tests::{GRILL, TDD, loaded, model, on_item, press, space};
use super::*;
use crate::model::Kind;
use crate::profile::Entry;
use crate::tui::widgets::testing::render_to_buffer;

fn change(install: Vec<Entry>, remove: Vec<&str>) -> Act {
    Act::Change {
        install,
        remove: remove.into_iter().map(String::from).collect(),
    }
}

#[test]
fn picking_asks_for_nothing_until_the_picks_are_applied() {
    let mut tab = loaded();
    on_item(&mut tab, TDD);

    let first = press(&mut tab, space());
    on_item(&mut tab, GRILL);
    let second = press(&mut tab, space());

    assert_eq!((first, second), (None, None));
}

#[test]
fn enter_applies_every_pick_as_one_change() {
    let mut tab = loaded();
    on_item(&mut tab, TDD);
    press(&mut tab, space());
    on_item(&mut tab, GRILL);
    press(&mut tab, space());

    let act = press(&mut tab, KeyCode::Enter);

    let grill = library_skill("o/r", "grill").entry;
    assert_eq!(act, Some(change(vec![grill], vec!["skill:tdd"])));
}

#[test]
fn picking_an_item_twice_cancels_the_pick() {
    let mut tab = loaded();
    on_item(&mut tab, GRILL);

    press(&mut tab, space());
    press(&mut tab, space());

    assert!(tab.pending.is_empty());
}

#[test]
fn escape_clears_the_picks_before_anything_else() {
    let mut tab = loaded();
    on_item(&mut tab, GRILL);
    press(&mut tab, space());

    press(&mut tab, KeyCode::Esc);

    assert!(tab.pending.is_empty());
}

#[test]
fn space_on_a_group_folds_it_and_picks_nothing() {
    let mut tab = loaded();

    press(&mut tab, space());

    assert_eq!(tab.outline.visible().len(), 4);
    assert!(tab.pending.is_empty());
}

#[test]
fn a_pick_survives_a_new_snapshot_while_the_item_is_unchanged() {
    let mut tab = loaded();
    on_item(&mut tab, GRILL);
    press(&mut tab, space());

    tab.load(&model());

    assert!(!tab.pending.is_empty());
}

#[test]
fn a_pick_is_dropped_when_its_item_is_gone_or_has_changed() {
    let mut tab = loaded();
    on_item(&mut tab, GRILL);
    press(&mut tab, space());
    let mut installed_now = model();
    installed_now
        .installed
        .push(installed("grill", Kind::Skill, Some("o/r")));

    tab.load(&installed_now);

    assert!(tab.pending.is_empty());
}

#[test]
fn a_section_counts_what_is_picked_in_it() {
    let mut tab = loaded();
    on_item(&mut tab, TDD);
    press(&mut tab, space());
    on_item(&mut tab, GRILL);
    press(&mut tab, space());
    let items = tab.rows.iter().filter_map(|row| match row {
        Row::Item(item) => Some(item),
        _ => None,
    });

    let counts = tab.pending.counts(items);

    assert_eq!(
        counts,
        pending::Counts {
            install: 1,
            remove: 1
        }
    );
}

fn picked_glyphs(glyphs: crate::tui::glyphs::Glyphs) -> (usize, usize) {
    let mut tab = ProjectTab::new();
    tab.load(&Model {
        dir: "app".into(),
        ..model()
    });
    on_item(&mut tab, TDD);
    press(&mut tab, space());
    on_item(&mut tab, GRILL);
    press(&mut tab, space());
    let mut theme = Theme::dark();
    theme.glyphs = glyphs;
    let buffer = render_to_buffer(80, 24, |f| tab.draw(f, f.area(), &theme));
    let count = |glyph: &str| {
        buffer
            .content()
            .iter()
            .filter(|c| c.symbol() == glyph)
            .count()
    };
    (count(glyphs.to_install), count(glyphs.to_remove))
}

#[test]
fn picked_rows_draw_their_own_marker_in_both_glyph_sets() {
    use crate::tui::glyphs::Glyphs;

    assert_eq!(picked_glyphs(Glyphs::UNICODE), (1, 1));
    assert_eq!(picked_glyphs(Glyphs::ASCII).1, 1);
}
