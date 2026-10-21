//! The screens the workspace pushes over itself: a yes/no question and a scrolling text report.

use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, MouseEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::Paragraph;

use super::app::{Screen, Step};
use super::keymap::{self, Action, Scope};
use super::theme::Theme;
use super::widgets::confirm::{ConfirmDialog, Outcome};
use super::widgets::panel::Panel;

const PAGE: usize = 10;

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

/// Text to read. With a question the answer is yes or no; without one any dismissal is `true`.
pub struct Review {
    title: String,
    lines: Vec<String>,
    question: Option<String>,
    offset: usize,
}

impl Review {
    pub fn new(title: &str, body: &str, question: Option<&str>) -> Self {
        Review {
            title: title.to_string(),
            lines: body.lines().map(str::to_string).collect(),
            question: question.map(str::to_string),
            offset: 0,
        }
    }

    fn scroll(&mut self, up: bool, by: usize) {
        let last = self.lines.len().saturating_sub(1);
        self.offset = if up {
            self.offset.saturating_sub(by)
        } else {
            (self.offset + by).min(last)
        };
    }

    fn on_key(&mut self, key: KeyEvent) -> Step<bool> {
        match (self.question.is_some(), key.code) {
            (true, KeyCode::Char('y' | 'Y')) => return Step::Act(true),
            (true, KeyCode::Char('n' | 'N') | KeyCode::Esc | KeyCode::Char('q')) => {
                return Step::Act(false);
            }
            (false, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q' | ' ')) => {
                return Step::Act(true);
            }
            _ => {}
        }
        match keymap::resolve(Scope::Common, key) {
            Some(Action::Up) => self.scroll(true, 1),
            Some(Action::Down) => self.scroll(false, 1),
            Some(Action::PageUp) => self.scroll(true, PAGE),
            Some(Action::PageDown) => self.scroll(false, PAGE),
            Some(Action::Top) => self.offset = 0,
            Some(Action::Bottom) => self.scroll(false, self.lines.len()),
            _ => {}
        }
        Step::Stay
    }
}

impl Screen<bool> for Review {
    fn handle(&mut self, event: &Event) -> Step<bool> {
        match event {
            Event::Key(key) => self.on_key(*key),
            Event::Mouse(mouse) => {
                match mouse.kind {
                    MouseEventKind::ScrollUp => self.scroll(true, 1),
                    MouseEventKind::ScrollDown => self.scroll(false, 1),
                    _ => {}
                }
                Step::Stay
            }
            _ => Step::Stay,
        }
    }

    fn draw(&mut self, frame: &mut Frame, theme: &Theme) {
        let [body, foot] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(frame.area());
        let inner = Panel::new(self.title.clone())
            .focused(true)
            .render(frame, body, theme);
        let text = self.lines.join("\n");
        let page = Paragraph::new(text).scroll((self.offset.min(u16::MAX as usize) as u16, 0));
        frame.render_widget(page, inner);
        let hint = match &self.question {
            Some(q) => format!("{q}  y yes  n no  ↑↓ scroll"),
            None => "esc close  ↑↓ scroll".to_string(),
        };
        frame.render_widget(Paragraph::new(hint).style(theme.muted), foot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::{key, keys};

    fn press(screen: &mut impl Screen<bool>, k: KeyEvent) -> Option<bool> {
        match screen.handle(&Event::Key(k)) {
            Step::Act(answer) => Some(answer),
            _ => None,
        }
    }

    fn long(asks: bool) -> Review {
        let body = (0..30)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        Review::new("t", &body, asks.then_some("q"))
    }

    #[test]
    fn a_question_answers_yes_and_no_and_esc_declines() {
        let answers: Vec<_> = [KeyCode::Char('y'), KeyCode::Char('n'), KeyCode::Esc]
            .into_iter()
            .map(|code| press(&mut long(true), key(code)))
            .collect();

        assert_eq!(answers, [Some(true), Some(false), Some(false)]);
    }

    #[test]
    fn enter_does_not_accept_a_question() {
        let answer = press(&mut long(true), key(KeyCode::Enter));

        // Only an explicit y says yes.
        assert_eq!(answer, None);
    }

    #[test]
    fn a_report_closes_on_esc_or_enter() {
        let answers = [KeyCode::Esc, KeyCode::Enter].map(|c| press(&mut long(false), key(c)));

        assert_eq!(answers, [Some(true), Some(true)]);
    }

    #[test]
    fn scrolling_moves_the_text_and_stops_at_the_ends() {
        let mut review = long(false);

        for k in keys("jjj") {
            press(&mut review, k);
        }
        let down = review.offset;
        for k in keys("kkkkk") {
            press(&mut review, k);
        }
        press(&mut review, key(KeyCode::End));

        // Three down, clamped at the top going up, and End lands on the last line.
        assert_eq!((down, review.offset), (3, 29));
    }
}
