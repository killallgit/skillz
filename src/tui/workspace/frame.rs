//! The border around the whole workspace: the app name on the left of the top edge and the
//! directory it is working in, dim, on the right.

use ratatui::Frame;
use ratatui::layout::{Margin, Rect};
use ratatui::widgets::Paragraph;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::tui::theme::Theme;
use crate::tui::widgets::margin;
use crate::tui::widgets::panel::Panel;

const TITLE: &str = "skillz";

/// Draw the frame inside the screen margin and return the area it holds, edge to edge.
pub fn draw(frame: &mut Frame, screen: Rect, dir: &str, theme: &Theme) -> Rect {
    let area = margin::inset(screen);
    Panel::new(TITLE)
        .focused(true)
        .badge(ellipsize_start(dir, dir_room(area.width)))
        .render(frame, area, theme)
}

/// `row` without the room to breathe: the text sits a cell off the side borders.
pub fn padded(row: Rect) -> Rect {
    row.inner(Margin::new(1, 0))
}

/// A rule across `row`, joined to the side borders just outside it.
pub fn divider(frame: &mut Frame, row: Rect, theme: &Theme) {
    let line = format!("├{}┤", "─".repeat(usize::from(row.width)));
    let area = Rect {
        x: row.x.saturating_sub(1),
        width: row.width + 2,
        ..row
    };
    frame.render_widget(Paragraph::new(line).style(theme.border_for(true)), area);
}

/// Columns the directory may take on the top edge: what the corners, the padding around both
/// labels and the title leave, and at least one cell of rule between them.
fn dir_room(frame_width: u16) -> usize {
    let taken = 2 + 2 + TITLE.width() + 2 + 1;
    usize::from(frame_width).saturating_sub(taken)
}

/// `text` cut from the front to fit `room` columns, so the end of a path stays readable.
fn ellipsize_start(text: &str, room: usize) -> String {
    if text.width() <= room {
        return text.to_string();
    }
    let mut kept: Vec<char> = Vec::new();
    let mut used = 1;
    for c in text.chars().rev() {
        let w = c.width().unwrap_or(0);
        if used + w > room {
            break;
        }
        used += w;
        kept.push(c);
    }
    kept.push('…');
    kept.iter().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_directory_too_long_for_the_edge_keeps_its_end() {
        assert_eq!(ellipsize_start("~/a/very/long/path", 8), "…ng/path");
        assert_eq!(ellipsize_start("~/short", 8), "~/short");
    }

    #[test]
    fn the_directory_gets_what_the_title_and_corners_leave() {
        assert_eq!(dir_room(60), 60 - 2 - 2 - TITLE.width() - 2 - 1);
        assert_eq!(dir_room(3), 0);
    }
}
