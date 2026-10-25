use ratatui::crossterm::event::KeyCode;

use super::tests::{loaded, press};
use super::*;
use crate::app::review::{Phase, Target};
use crate::tui::glyphs::Glyphs;
use crate::tui::widgets::testing::render_to_buffer;

const WIDTH: u16 = 80;

const TDD_ID: &str = "skill:tdd";

fn event(phase: Phase) -> ItemEvent {
    ItemEvent {
        target: Target::Remove(TDD_ID.into()),
        label: "tdd".into(),
        phase,
    }
}

/// The text of the screen row that shows `label`.
fn row_of(tab: &mut InstalledTab, glyphs: Glyphs, label: &str) -> String {
    let theme = Theme {
        glyphs,
        ..Theme::dark()
    };
    let buffer = render_to_buffer(WIDTH, 24, |f| tab.draw(f, f.area(), &theme));
    buffer
        .content()
        .chunks(usize::from(WIDTH))
        .map(|cells| cells.iter().map(|c| c.symbol()).collect::<String>())
        .find(|line| line.contains(label))
        .unwrap()
}

fn mark_before_label(row: &str, mark: &str) -> bool {
    matches!((row.find(mark), row.find("tdd")), (Some(m), Some(l)) if m < l)
}

#[test]
fn a_row_in_flight_shows_the_spinner_left_of_its_name_in_both_glyph_sets() {
    for glyphs in [Glyphs::UNICODE, Glyphs::ASCII] {
        let mut tab = loaded();
        tab.note(&event(Phase::Started));
        let frame = glyphs.frame(tab.statuses.ticks());

        let row = row_of(&mut tab, glyphs, "tdd");

        assert!(mark_before_label(&row, frame));
    }
}

#[test]
fn a_finished_row_shows_a_tick_or_a_cross_instead() {
    for (phase, mark) in [
        (Phase::Succeeded, Glyphs::ASCII.loaded),
        (Phase::Failed("no".into()), Glyphs::ASCII.failed),
    ] {
        let mut tab = loaded();
        tab.note(&event(phase));

        let row = row_of(&mut tab, Glyphs::ASCII, "tdd");

        assert!(mark_before_label(&row, mark));
    }
}

#[test]
fn the_marks_go_away_on_the_next_key() {
    let mut tab = loaded();
    tab.note(&event(Phase::Failed("no".into())));

    press(&mut tab, KeyCode::Down);

    assert_eq!(tab.statuses.of(TDD_ID), None);
}

#[test]
fn the_marks_go_away_with_the_next_snapshot() {
    let mut tab = loaded();
    tab.note(&event(Phase::Succeeded));

    tab.load(&super::tests::model());

    assert_eq!(tab.statuses.of(TDD_ID), None);
}
