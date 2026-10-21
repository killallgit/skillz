//! A yes/no dialog. Focus starts on No so a stray Enter never destroys anything.

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use super::modal::{self, ModalSize};
use super::panel::Panel;
use crate::tui::theme::Theme;

/// The dialog's verdict after a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Pending,
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Button {
    Yes,
    No,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmDialog {
    title: String,
    prompt: String,
    focus: Button,
}

impl ConfirmDialog {
    pub fn new(title: impl Into<String>, prompt: impl Into<String>) -> Self {
        ConfirmDialog {
            title: title.into(),
            prompt: prompt.into(),
            focus: Button::No,
        }
    }

    #[cfg(test)]
    pub fn yes_focused(&self) -> bool {
        self.focus == Button::Yes
    }

    pub fn handle(&mut self, key: KeyEvent) -> Outcome {
        match key.code {
            KeyCode::Char('y' | 'Y') => Outcome::Yes,
            KeyCode::Char('n' | 'N') | KeyCode::Esc => Outcome::No,
            KeyCode::Enter => self.choice(),
            KeyCode::Tab | KeyCode::BackTab | KeyCode::Left | KeyCode::Right => {
                self.toggle_focus();
                Outcome::Pending
            }
            _ => Outcome::Pending,
        }
    }

    fn choice(&self) -> Outcome {
        match self.focus {
            Button::Yes => Outcome::Yes,
            Button::No => Outcome::No,
        }
    }

    fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Button::Yes => Button::No,
            Button::No => Button::Yes,
        };
    }

    pub fn render(&self, frame: &mut Frame, theme: &Theme) {
        let size = ModalSize {
            width: 52,
            height: 7,
        };
        let inner = modal::open(frame, size, Panel::new(self.title.clone()), theme);
        let [text, _, buttons] = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(inner);
        let prompt = Paragraph::new(self.prompt.clone()).wrap(Wrap { trim: true });
        frame.render_widget(prompt, text);
        let row = Paragraph::new(self.button_row(theme)).centered();
        frame.render_widget(row, buttons);
    }

    fn button_row(&self, theme: &Theme) -> Line<'static> {
        Line::from(vec![
            self.button("Yes", Button::Yes, theme),
            Span::raw("   "),
            self.button("No", Button::No, theme),
        ])
    }

    fn button(&self, label: &str, which: Button, theme: &Theme) -> Span<'static> {
        let base = theme.text;
        let style = if self.focus == which {
            base.patch(Style::new().add_modifier(Modifier::REVERSED | Modifier::BOLD))
        } else {
            base
        };
        Span::styled(format!("[ {label} ]"), style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::{key, render_to_buffer, row_text};

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

    fn button_cell(d: &ConfirmDialog, theme: &Theme) -> ratatui::buffer::Cell {
        let buffer = render_to_buffer(60, 11, |f| d.render(f, theme));
        let y = (0..11)
            .find(|y| row_text(&buffer, *y).contains('['))
            .unwrap();
        let x = row_text(&buffer, y).chars().position(|c| c == '[').unwrap();
        buffer[(x as u16, y)].clone()
    }

    #[test]
    fn focused_button_is_reversed() {
        let theme = Theme::dark();
        let mut d = dialog();
        d.handle(key(KeyCode::Tab));
        let cell = button_cell(&d, &theme);
        assert!(cell.modifier.contains(Modifier::REVERSED));
    }
}
