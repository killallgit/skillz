//! A confirm dialog: a question and two answers as buttons, the cancelling one first. Focus starts
//! on the cancelling button so a stray Enter never destroys anything.

use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::widgets::{Paragraph, Wrap};

use super::buttons::Buttons;
use super::container::{Container, Size};
use super::modal::{self, ModalSize};
use super::panel::Panel;
use crate::tui::keymap::{self, Action, Scope};
use crate::tui::theme::Theme;

/// The dialog's verdict after a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Pending,
    Yes,
    No,
}

/// Where the cancelling and the confirming button sit in the row.
const CANCEL: usize = 0;
const CONFIRM: usize = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmDialog {
    title: String,
    prompt: String,
    buttons: Buttons,
}

impl ConfirmDialog {
    pub fn new(title: impl Into<String>, prompt: impl Into<String>) -> Self {
        ConfirmDialog {
            title: title.into(),
            prompt: prompt.into(),
            buttons: Buttons::new(&["No", "Yes"], CANCEL),
        }
    }

    #[cfg(test)]
    pub fn yes_focused(&self) -> bool {
        self.buttons.focus() == CONFIRM
    }

    pub fn handle(&mut self, key: KeyEvent) -> Outcome {
        match keymap::resolve(Scope::Confirm, key) {
            Some(Action::Yes) => Outcome::Yes,
            Some(Action::No | Action::Back | Action::Quit | Action::Interrupt) => Outcome::No,
            Some(Action::Select) => self.choice(),
            Some(Action::Toggle) => {
                self.buttons.next();
                Outcome::Pending
            }
            _ => Outcome::Pending,
        }
    }

    fn choice(&self) -> Outcome {
        match self.buttons.focus() {
            CONFIRM => Outcome::Yes,
            _ => Outcome::No,
        }
    }

    pub fn render(&self, frame: &mut Frame, theme: &Theme) {
        let size = ModalSize {
            width: 52,
            height: 7,
        };
        let inner = modal::open(frame, size, Panel::new(self.title.clone()), theme);
        let [text, _, buttons] =
            Container::column([Size::Fill(1), Size::Fixed(1), Size::Fixed(1)]).areas(inner);
        let prompt = Paragraph::new(self.prompt.clone()).wrap(Wrap { trim: true });
        frame.render_widget(prompt, text);
        frame.render_widget(Paragraph::new(self.buttons.line(theme)).centered(), buttons);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::{key, render_to_buffer, row_text};
    use ratatui::crossterm::event::KeyCode;
    use ratatui::style::Modifier;

    fn dialog() -> ConfirmDialog {
        ConfirmDialog::new("Remove", "Sure?")
    }

    #[test]
    fn focus_starts_on_no() {
        assert!(!dialog().yes_focused());
    }

    #[test]
    fn enter_on_the_default_declines() {
        assert_eq!(dialog().handle(key(KeyCode::Enter)), Outcome::No);
    }

    #[test]
    fn tab_moves_focus_to_yes_then_enter_accepts() {
        let mut d = dialog();
        assert_eq!(d.handle(key(KeyCode::Tab)), Outcome::Pending);
        assert_eq!(d.handle(key(KeyCode::Enter)), Outcome::Yes);
    }

    #[test]
    fn arrows_toggle_focus_both_ways() {
        let mut d = dialog();
        d.handle(key(KeyCode::Left));
        assert!(d.yes_focused());
        d.handle(key(KeyCode::Right));
        assert!(!d.yes_focused());
    }

    #[test]
    fn y_accepts_immediately() {
        assert_eq!(dialog().handle(key(KeyCode::Char('y'))), Outcome::Yes);
    }

    #[test]
    fn n_and_escape_decline() {
        assert_eq!(dialog().handle(key(KeyCode::Char('n'))), Outcome::No);
        assert_eq!(dialog().handle(key(KeyCode::Esc)), Outcome::No);
    }

    #[test]
    fn unrelated_keys_leave_it_pending() {
        assert_eq!(dialog().handle(key(KeyCode::Char('z'))), Outcome::Pending);
    }

    fn drawn(d: &ConfirmDialog, theme: &Theme) -> ratatui::buffer::Buffer {
        render_to_buffer(60, 12, |f| d.render(f, theme))
    }

    fn button_cell(d: &ConfirmDialog, theme: &Theme) -> ratatui::buffer::Cell {
        let buffer = drawn(d, theme);
        let y = (0..12)
            .find(|y| row_text(&buffer, *y).contains('['))
            .unwrap();
        let x = row_text(&buffer, y).chars().position(|c| c == '[').unwrap();
        buffer[(x as u16, y)].clone()
    }

    #[test]
    fn focused_button_is_reversed() {
        let theme = Theme::dark();
        let mut d = dialog();
        // The cancelling button is first; moving on focuses the confirming one, so the first
        // button is no longer the reversed one.
        let before = button_cell(&d, &theme);
        d.handle(key(KeyCode::Tab));
        let after = button_cell(&d, &theme);
        assert!(before.modifier.contains(Modifier::REVERSED));
        assert!(!after.modifier.contains(Modifier::REVERSED));
    }
}
