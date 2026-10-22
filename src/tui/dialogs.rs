//! A yes/no question shown on the shared terminal.

use ratatui::Frame;
use ratatui::crossterm::event::Event;

use super::app::{Screen, Step};
use super::theme::Theme;
use super::widgets::confirm::{ConfirmDialog, Outcome};

/// A yes/no question; the answer is the act.
pub struct Ask(pub ConfirmDialog);

impl Screen<bool> for Ask {
    fn handle(&mut self, event: &Event) -> Step<bool> {
        let Event::Key(key) = event else {
            return Step::Stay;
        };
        match self.0.handle(*key) {
            Outcome::Yes => Step::Act(true),
            Outcome::No => Step::Act(false),
            Outcome::Pending => Step::Stay,
        }
    }

    fn draw(&mut self, frame: &mut Frame, theme: &Theme) {
        self.0.render(frame, theme);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::key;
    use ratatui::crossterm::event::KeyCode;

    fn answer(code: KeyCode) -> Option<bool> {
        let mut ask = Ask(ConfirmDialog::new("t", "q"));
        match ask.handle(&Event::Key(key(code))) {
            Step::Act(answer) => Some(answer),
            _ => None,
        }
    }

    #[test]
    fn y_and_n_answer_and_other_keys_wait() {
        let answers = [KeyCode::Char('y'), KeyCode::Char('n'), KeyCode::Char('x')].map(answer);

        assert_eq!(answers, [Some(true), Some(false), None]);
    }
}
