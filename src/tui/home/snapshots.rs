//! The workspace drawn on an in-memory terminal at three widths in each theme, checked by its
//! structure: how many panes, which border is lit, where the cursor row sits.

use ratatui::buffer::Buffer;
use ratatui::symbols::border::ROUNDED;

use super::*;
use crate::tui::theme::Theme;
use crate::tui::widgets::testing::{has_style, render_to_buffer, row_text};

const HEIGHT: u16 = 24;

fn model() -> Model {
    let project = |name: &str| ProjectRow {
        path: name.into(),
        profile: None,
        short: name.into(),
        missing: false,
        detail: vec![],
    };
    let profile = |name: &str| ProfileRow {
        name: name.into(),
        detail: vec![],
        broken: false,
    };
    Model {
        places: Default::default(),
        current: None,
        projects: vec![project("a"), project("b")],
        profiles: vec![profile("x")],
        notice: None,
        undo: None,
    }
}

fn themes() -> [Theme; 3] {
    [Theme::dark(), Theme::light(), Theme::mono()]
}

fn draw(width: u16, theme: &Theme) -> Buffer {
    let mut home = Home::new(model(), Pos::default());
    render_to_buffer(width, HEIGHT, |f| home.draw(f, theme))
}

fn corners(buffer: &Buffer, y: u16) -> Vec<u16> {
    (0..buffer.area.width)
        .filter(|&x| buffer[(x, y)].symbol() == ROUNDED.top_left)
        .collect()
}

fn blank(buffer: &Buffer, y: u16) -> bool {
    row_text(buffer, y).trim().is_empty()
}

#[test]
fn the_pane_count_follows_the_width() {
    for theme in themes() {
        let wide = draw(120, &theme);
        let medium = draw(80, &theme);
        let narrow = draw(40, &theme);

        // Three panes side by side, two columns, and one pane under the tab row.
        assert_eq!(corners(&wide, 1).len(), 3);
        assert_eq!(corners(&medium, 1).len(), 2);
        assert_eq!(
            (corners(&narrow, 1).len(), corners(&narrow, 2).len()),
            (0, 1)
        );
    }
}

#[test]
fn the_header_and_key_bar_frame_the_panes_at_every_width() {
    for theme in themes() {
        for width in [40, 80, 120] {
            let buffer = draw(width, &theme);

            // The first and last rows carry content; the panes sit between them.
            assert!(!blank(&buffer, 0));
            assert!(!blank(&buffer, HEIGHT - 1));
        }
    }
}

#[test]
fn the_focused_pane_has_the_lit_border_and_the_others_do_not() {
    for theme in themes() {
        let buffer = draw(120, &theme);
        let at = corners(&buffer, 1);

        let lit = has_style(&buffer[(at[0], 1)], theme.border_for(true));
        let dim = has_style(&buffer[(at[1], 1)], theme.border_for(false));
        let same_as_lit = buffer[(at[1], 1)].style() == buffer[(at[0], 1)].style();

        // The projects pane has focus; the profiles pane differs from it in style.
        assert!(lit && dim && !same_as_lit);
    }
}

#[test]
fn the_cursor_row_carries_the_selection_style_in_every_theme() {
    for theme in themes() {
        for (width, row) in [(120, 2), (80, 2), (40, 3)] {
            let buffer = draw(width, &theme);

            let on_cursor = has_style(&buffer[(6, row)], theme.selection);
            let off_cursor = has_style(&buffer[(6, row + 1)], theme.selection);

            // The first project is selected and the one below it is not.
            assert!(on_cursor && !off_cursor);
        }
    }
}

#[test]
fn a_terminal_below_the_minimum_draws_no_panes() {
    for theme in themes() {
        let buffer = draw(30, &theme);

        let panes: usize = (0..HEIGHT).map(|y| corners(&buffer, y).len()).sum();

        assert_eq!(panes, 0);
    }
}
