//! How a switch ended: counts, then anything that went wrong with a key to copy it or run the
//! rest again, then the steps that ran.

use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use super::app::{Screen, Step};
use super::clipboard;
use super::theme::Theme;
use super::widgets::modal::{self, ModalSize};
use super::widgets::panel::Panel;
use super::widgets::scroll::Scroll;
use crate::review::{Next, Outcome};

const WIDTH: u16 = 76;
const CHROME: u16 = 4;

pub struct ResultScreen {
    title: String,
    outcome: Outcome,
    scroll: Scroll,
    copied: bool,
    shown: usize,
    copy: fn(&str),
}

impl ResultScreen {
    pub fn new(title: &str, outcome: Outcome) -> Self {
        ResultScreen {
            title: title.to_string(),
            outcome,
            scroll: Scroll::default(),
            copied: false,
            shown: 0,
            copy: copy_to_terminal,
        }
    }

    fn can_retry(&self) -> bool {
        !self.outcome.failures.is_empty()
    }

    /// What `c` copies: the problems when there are any, otherwise the log.
    fn clip(&self) -> String {
        if self.outcome.is_clean() {
            self.outcome.log.join("\n")
        } else {
            self.outcome.problems()
        }
    }

    fn on_key(&mut self, key: KeyEvent) -> Step<Next> {
        match key.code {
            KeyCode::Char('c') => {
                (self.copy)(&self.clip());
                self.copied = true;
            }
            KeyCode::Char('r') if self.can_retry() => return Step::Act(Next::Retry),
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q' | ' ') => {
                return Step::Act(Next::Done);
            }
            _ => {
                self.scroll.key(key, self.shown);
            }
        }
        Step::Stay
    }

    fn lines(&self, theme: &Theme) -> Vec<Line<'static>> {
        let mut lines = vec![counts(&self.outcome, theme)];
        lines.extend(problem_lines(&self.outcome, theme));
        if !self.outcome.log.is_empty() {
            lines.push(Line::default());
            lines.push(Line::styled("steps", theme.muted));
            lines.extend(self.outcome.log.iter().map(|l| Line::raw(format!("  {l}"))));
        }
        lines
    }

    fn hints(&self, theme: &Theme) -> Line<'static> {
        let mut spans = vec![Span::styled("enter close  ", theme.accent)];
        if self.can_retry() {
            spans.push(Span::styled("r retry  ", theme.warn));
        }
        let copy = if self.copied {
            "c copied  "
        } else {
            "c copy  "
        };
        spans.push(Span::styled(copy, theme.muted));
        spans.push(Span::styled("↑↓ scroll", theme.muted));
        Line::from(spans)
    }
}

fn copy_to_terminal(text: &str) {
    let _ = clipboard::copy(text);
}

fn counts(outcome: &Outcome, theme: &Theme) -> Line<'static> {
    let count = |n: usize, what: &str, shown: ratatui::style::Style| {
        let style = if n == 0 { theme.muted } else { shown };
        Span::styled(format!("{what} {n}   "), style)
    };
    Line::from(vec![
        count(outcome.removed, "removed", theme.text),
        count(outcome.installed, "installed", theme.ok),
        count(outcome.updated, "updated", theme.ok),
        count(outcome.skipped, "skipped", theme.warn),
        count(outcome.failures.len(), "failed", theme.danger),
    ])
}

fn problem_lines(outcome: &Outcome, theme: &Theme) -> Vec<Line<'static>> {
    if outcome.is_clean() {
        return vec![];
    }
    let mut lines = vec![Line::default()];
    let marked = |text: &str, marker: &str| {
        Line::from(vec![
            Span::styled(format!("{marker} "), theme.danger),
            Span::raw(text.to_string()),
        ])
    };
    lines.extend(
        outcome
            .failures
            .iter()
            .map(|f| marked(f, theme.glyphs.broken)),
    );
    let left = |name: &String| format!("still installed after removal: {name}");
    lines.extend(
        outcome
            .still_installed
            .iter()
            .map(|n| marked(&left(n), theme.glyphs.broken)),
    );
    lines
}

impl Screen<Next> for ResultScreen {
    fn handle(&mut self, event: &Event) -> Step<Next> {
        match event {
            Event::Key(key) => self.on_key(*key),
            other => {
                self.scroll.wheel(other, self.shown);
                Step::Stay
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame, theme: &Theme) {
        let text = self.lines(theme);
        self.shown = text.len();
        let size = ModalSize {
            width: WIDTH,
            height: (text.len() as u16).saturating_add(CHROME),
        };
        let panel = Panel::new(self.title.clone())
            .focused(true)
            .danger(!self.outcome.is_clean());
        let inner = modal::open(frame, size, panel, theme);
        let [body, foot] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
        let page = Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll.offset(), 0));
        frame.render_widget(page, body);
        frame.render_widget(Paragraph::new(self.hints(theme)), foot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::key;
    use std::cell::RefCell;

    thread_local! {
        static COPIED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }

    fn remember(text: &str) {
        COPIED.with(|c| c.borrow_mut().push(text.to_string()));
    }

    fn failed() -> Outcome {
        Outcome {
            failures: vec!["boom".into()],
            still_installed: vec!["x".into()],
            log: vec!["step".into()],
            ..Outcome::default()
        }
    }

    fn answer(screen: &mut ResultScreen, code: KeyCode) -> Option<Next> {
        match screen.handle(&Event::Key(key(code))) {
            Step::Act(next) => Some(next),
            _ => None,
        }
    }

    #[test]
    fn retry_is_offered_only_after_a_failure() {
        let clean = answer(
            &mut ResultScreen::new("t", Outcome::default()),
            KeyCode::Char('r'),
        );
        let after_failure = answer(&mut ResultScreen::new("t", failed()), KeyCode::Char('r'));

        // A clean run ignores r; a failed one retries.
        assert_eq!((clean, after_failure), (None, Some(Next::Retry)));
    }

    #[test]
    fn copy_sends_the_problems_and_marks_it_copied() {
        let mut screen = ResultScreen::new("t", failed());
        screen.copy = remember;

        answer(&mut screen, KeyCode::Char('c'));

        // The failure and the leftover are on the clipboard, one per line.
        let sent = COPIED.with(|c| c.borrow().clone());
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].lines().count(), 2);
        assert!(screen.copied);
    }

    #[test]
    fn a_clean_outcome_copies_its_log() {
        let outcome = Outcome {
            log: vec!["a".into(), "b".into()],
            ..Outcome::default()
        };
        let screen = ResultScreen::new("t", outcome);

        let clip = screen.clip();

        assert_eq!(clip.lines().count(), 2);
    }

    #[test]
    fn problems_are_drawn_in_the_danger_colour() {
        let theme = Theme::dark();
        let screen = ResultScreen::new("t", failed());

        let lines = screen.lines(&theme);

        // Line 0 is the counts; the failure is the second problem block line.
        assert_eq!(lines[2].spans[0].style, theme.danger);
    }
}
