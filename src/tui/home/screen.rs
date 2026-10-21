//! The workspace as a screen on the app's stack.

use std::time::Instant;

use ratatui::Frame;
use ratatui::crossterm::event::Event;

use super::model::Act;
use super::state::Home;
use crate::tui::app::{Screen, Step};
use crate::tui::theme::Theme;

/// Borrows the home so the caller can read its cursors after the stack has run.
pub struct HomeScreen<'a>(pub &'a mut Home);

impl Screen<Act> for HomeScreen<'_> {
    fn handle(&mut self, event: &Event) -> Step<Act> {
        let act = match event {
            Event::Key(key) => self.0.handle(*key),
            Event::Mouse(mouse) => self.0.on_mouse(*mouse, Instant::now()),
            _ => None,
        };
        act.map_or(Step::Stay, Step::Act)
    }

    fn draw(&mut self, frame: &mut Frame, theme: &Theme) {
        self.0.draw(frame, theme);
    }
}
