//! A vertical scroll offset for read-only text, driven by the shared movement keys and the wheel.

use ratatui::crossterm::event::{Event, KeyEvent, MouseEventKind};

use crate::tui::keymap::{self, Action, Scope};

const PAGE: usize = 10;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Scroll {
    offset: usize,
}

impl Scroll {
    /// The first line to draw, for `Paragraph::scroll`.
    pub fn offset(&self) -> u16 {
        self.offset.min(u16::MAX as usize) as u16
    }

    /// Move for a movement key over `len` lines; true when the key was a movement.
    pub fn key(&mut self, key: KeyEvent, len: usize) -> bool {
        let Some(action) = keymap::resolve(Scope::Common, key) else {
            return false;
        };
        match action {
            Action::Up => self.up(1),
            Action::Down => self.down(1, len),
            Action::PageUp => self.up(PAGE),
            Action::PageDown => self.down(PAGE, len),
            Action::Top => self.offset = 0,
            Action::Bottom => self.down(len, len),
            _ => return false,
        }
        true
    }

    /// Move for the mouse wheel; true when the event was one.
    pub fn wheel(&mut self, event: &Event, len: usize) -> bool {
        let Event::Mouse(mouse) = event else {
            return false;
        };
        match mouse.kind {
            MouseEventKind::ScrollUp => self.up(1),
            MouseEventKind::ScrollDown => self.down(1, len),
            _ => return false,
        }
        true
    }

    fn up(&mut self, by: usize) {
        self.offset = self.offset.saturating_sub(by);
    }

    fn down(&mut self, by: usize, len: usize) {
        self.offset = (self.offset + by).min(len.saturating_sub(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::key;
    use ratatui::crossterm::event::KeyCode;

    #[test]
    fn moving_stops_at_both_ends() {
        let mut scroll = Scroll::default();

        scroll.key(key(KeyCode::Up), 5);
        let top = scroll.offset();
        scroll.key(key(KeyCode::End), 5);

        // Up from the top stays put; End lands on the last of five lines.
        assert_eq!((top, scroll.offset()), (0, 4));
    }

    #[test]
    fn keys_that_are_not_movements_are_left_alone() {
        let mut scroll = Scroll::default();

        let moved = scroll.key(key(KeyCode::Char('x')), 5);

        assert!(!moved);
    }
}
