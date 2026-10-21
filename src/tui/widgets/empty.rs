//! A centred message for a pane with nothing to list: what is missing and the key that fixes it.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

/// Draw `lines` centred in `area`, vertically too.
pub fn render(frame: &mut Frame, area: Rect, lines: Vec<Line<'static>>) {
    let height = (lines.len() as u16).min(area.height);
    let top = area.y + (area.height - height) / 2;
    let target = Rect::new(area.x, top, area.width, height);
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), target);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::{render_to_buffer, row_text};

    fn first_row_with_text(height: u16, lines: usize) -> Option<u16> {
        let buffer = render_to_buffer(20, height, |f| {
            render(f, f.area(), vec![Line::from("x"); lines]);
        });
        (0..height).find(|&y| row_text(&buffer, y).contains('x'))
    }

    #[test]
    fn the_message_sits_in_the_vertical_middle() {
        assert_eq!(first_row_with_text(9, 3), Some(3));
    }

    #[test]
    fn a_message_taller_than_the_area_starts_at_the_top() {
        assert_eq!(first_row_with_text(2, 5), Some(0));
    }
}
