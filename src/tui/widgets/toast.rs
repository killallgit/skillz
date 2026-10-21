//! Transient notices. Expiry is counted in ticks the caller supplies, so nothing reads a clock.
//! Errors stay until dismissed.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use crate::tui::theme::{State, Theme};

const LIFETIME_TICKS: u64 = 30;
const MAX_SHOWN: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Success,
    Warn,
    Error,
}

impl Level {
    fn state(self) -> State {
        match self {
            Level::Info => State::Muted,
            Level::Success => State::Ok,
            Level::Warn => State::Warn,
            Level::Error => State::Danger,
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            Level::Info => "i",
            Level::Success => "✓",
            Level::Warn => "!",
            Level::Error => "✗",
        }
    }

    fn expires(self) -> bool {
        self != Level::Error
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub level: Level,
    pub text: String,
    born: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Toasts {
    now: u64,
    queue: Vec<Toast>,
}

impl Toasts {
    pub fn push(&mut self, level: Level, text: impl Into<String>) {
        let toast = Toast {
            level,
            text: text.into(),
            born: self.now,
        };
        self.queue.push(toast);
    }

    /// Advance the clock to `now` and drop toasts that have outlived their lifetime.
    pub fn tick(&mut self, now: u64) {
        self.now = now;
        self.queue
            .retain(|t| !t.level.expires() || now.saturating_sub(t.born) < LIFETIME_TICKS);
    }

    /// Remove the oldest toast (the one on top), whatever its level.
    pub fn dismiss(&mut self) {
        if !self.queue.is_empty() {
            self.queue.remove(0);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn has_error(&self) -> bool {
        self.queue.iter().any(|t| t.level == Level::Error)
    }

    /// Stack the newest toasts up from the bottom-right corner of `area`.
    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let shown = self.queue.iter().rev().take(MAX_SHOWN);
        for (slot, toast) in shown.enumerate() {
            let Some(rect) = slot_rect(area, toast, slot) else {
                break;
            };
            frame.render_widget(Clear, rect);
            frame.render_widget(Paragraph::new(toast_line(toast, theme)), rect);
        }
    }
}

fn toast_line(toast: &Toast, theme: &Theme) -> Line<'static> {
    let style = theme.state_style(toast.level.state());
    Line::from(vec![
        Span::styled(format!(" {} ", toast.level.glyph()), style),
        Span::styled(format!("{} ", toast.text), theme.text),
    ])
}

fn slot_rect(area: Rect, toast: &Toast, slot: usize) -> Option<Rect> {
    let width = (toast.text.chars().count() as u16 + 5).min(area.width);
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
    fn info_expires_after_its_lifetime() {
        let mut toasts = Toasts::default();
        toasts.push(Level::Info, "hi");
        toasts.tick(LIFETIME_TICKS);
        assert!(toasts.is_empty());
    }

    #[test]
    fn info_survives_before_its_lifetime() {
        let mut toasts = Toasts::default();
        toasts.push(Level::Info, "hi");
        toasts.tick(LIFETIME_TICKS - 1);
        assert_eq!(toasts.len(), 1);
    }

    #[test]
    fn errors_persist_past_any_lifetime() {
        let mut toasts = Toasts::default();
        toasts.push(Level::Error, "boom");
        toasts.tick(LIFETIME_TICKS * 10);
        assert!(toasts.has_error());
    }

    #[test]
    fn lifetime_counts_from_when_the_toast_was_pushed() {
        let mut toasts = Toasts::default();
        toasts.tick(100);
        toasts.push(Level::Success, "ok");
        toasts.tick(100 + LIFETIME_TICKS - 1);
        assert_eq!(toasts.len(), 1);
    }

    #[test]
    fn dismiss_removes_the_oldest_even_an_error() {
        let mut toasts = Toasts::default();
        toasts.push(Level::Error, "boom");
        toasts.push(Level::Info, "hi");
        toasts.dismiss();
        assert!(!toasts.has_error());
        assert_eq!(toasts.len(), 1);
    }

    #[test]
    fn dismiss_on_empty_queue_is_harmless() {
        let mut toasts = Toasts::default();
        toasts.dismiss();
        assert!(toasts.is_empty());
    }

    #[test]
    fn newest_toast_sits_on_the_bottom_row_in_its_level_style() {
        let theme = Theme::dark();
        let mut toasts = Toasts::default();
        toasts.push(Level::Warn, "careful");
        let buffer = render_to_buffer(30, 4, |f| toasts.render(f, f.area(), &theme));
        let x = 30 - ("careful".len() as u16 + 5) + 1;
        assert_eq!(buffer[(x, 3)].fg, theme.warn.fg.unwrap());
    }

    #[test]
    fn older_toasts_stack_above_newer_ones() {
        let theme = Theme::dark();
        let mut toasts = Toasts::default();
        toasts.push(Level::Error, "older");
        toasts.push(Level::Success, "newer");
        let buffer = render_to_buffer(30, 4, |f| toasts.render(f, f.area(), &theme));
        assert_eq!(buffer[(29 - 9, 2)].fg, theme.danger.fg.unwrap());
        assert_eq!(buffer[(29 - 9, 3)].fg, theme.ok.fg.unwrap());
    }
}
