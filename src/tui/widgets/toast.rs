//! Transient notices, stacked in the bottom-right corner until dismissed.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use crate::tui::theme::{State, Theme};

const MAX_SHOWN: usize = 3;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Toasts {
    queue: Vec<String>,
}

impl Toasts {
    pub fn push(&mut self, text: impl Into<String>) {
        self.queue.push(text.into());
    }

    /// Remove the oldest toast (the one on top).
    pub fn dismiss(&mut self) {
        if !self.queue.is_empty() {
            self.queue.remove(0);
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Stack the newest toasts up from the bottom-right corner of `area`.
    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let shown = self.queue.iter().rev().take(MAX_SHOWN);
        for (slot, text) in shown.enumerate() {
            let Some(rect) = slot_rect(area, text, slot) else {
                break;
            };
            frame.render_widget(Clear, rect);
            frame.render_widget(Paragraph::new(toast_line(text, theme)), rect);
        }
    }
}

fn toast_line(text: &str, theme: &Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(" i ", theme.state_style(State::Muted)),
        Span::styled(format!("{text} "), theme.text),
    ])
}

fn slot_rect(area: Rect, text: &str, slot: usize) -> Option<Rect> {
    let width = (text.chars().count() as u16 + 5).min(area.width);
    let row_from_bottom = slot as u16 + 1;
    (area.height >= row_from_bottom).then(|| Rect {
        x: area.right() - width,
        y: area.bottom() - row_from_bottom,
        width,
        height: 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::render_to_buffer;

    #[test]
    fn dismiss_removes_the_oldest() {
        let mut toasts = Toasts::default();
        toasts.push("first");
        toasts.push("second");

        toasts.dismiss();

        // One toast is left after dismissing the oldest of two.
        assert_eq!(toasts.len(), 1);
    }

    #[test]
    fn dismiss_on_empty_queue_is_harmless() {
        let mut toasts = Toasts::default();

        toasts.dismiss();

        // Nothing queued, nothing to remove, no panic.
        assert_eq!(toasts.len(), 0);
    }

    #[test]
    fn newest_toast_sits_on_the_bottom_row_in_the_muted_style() {
        let theme = Theme::dark();
        let mut toasts = Toasts::default();
        toasts.push("careful");

        let buffer = render_to_buffer(30, 4, |f| toasts.render(f, f.area(), &theme));

        // The glyph cell on the bottom row carries the muted foreground.
        let x = 30 - ("careful".len() as u16 + 5) + 1;
        assert_eq!(buffer[(x, 3)].fg, theme.muted.fg.unwrap());
    }

    #[test]
    fn older_toasts_stack_above_newer_ones() {
        let theme = Theme::dark();
        let mut toasts = Toasts::default();
        toasts.push("older");
        toasts.push("newer");

        let buffer = render_to_buffer(30, 4, |f| toasts.render(f, f.area(), &theme));

        // Both rows are drawn: the newer on the bottom row, the older directly above it.
        let x = 30 - 10 + 1;
        assert_eq!(
            (buffer[(x, 2)].symbol(), buffer[(x, 3)].symbol()),
            ("i", "i")
        );
    }
}
