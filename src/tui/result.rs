//! How a switch ended: counts, then anything that went wrong with a key to copy it or run the
//! rest again, then the steps that ran.

use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyEvent};
use ratatui::text::{Line, Span};

use super::app::{Screen, Step};
use super::clipboard;
use super::keymap::{self, Action, Scope};
use super::result_items::done_lines;
use super::theme::Theme;
use super::widgets::panel::Panel;
use super::widgets::text_modal::{Page, TextModal};
use crate::review::{Next, Outcome};

pub struct ResultScreen {
    title: String,
    outcome: Outcome,
    modal: TextModal,
    copied: bool,
    copy: fn(&str),
}

impl ResultScreen {
    pub fn new(title: &str, outcome: Outcome) -> Self {
        ResultScreen {
            title: title.to_string(),
            outcome,
            modal: TextModal::default(),
            copied: false,
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
        match keymap::resolve(Scope::Result, key) {
            Some(Action::Copy) => {
                (self.copy)(&self.clip());
                self.copied = true;
            }
            Some(Action::Retry) if self.can_retry() => return Step::Act(Next::Retry),
            Some(Action::Back | Action::Select | Action::Quit) => {
                return Step::Act(Next::Done);
            }
            Some(Action::Interrupt) => return Step::Quit,
            Some(movement) => self.modal.navigate(movement),
            None => {}
        }
        Step::Stay
    }

    fn lines(&self, theme: &Theme) -> Vec<Line<'static>> {
        let mut lines = vec![counts(&self.outcome, theme)];
        lines.extend(done_lines(&self.outcome, theme));
        lines.extend(problem_lines(&self.outcome, theme));
        let steps = collapse_repeats(&self.outcome.log);
        if !self.outcome.is_clean() && !steps.is_empty() {
            lines.push(Line::default());
            lines.push(Line::styled("steps", theme.muted));
            lines.extend(steps.iter().map(|l| Line::raw(format!("  {l}"))));
        }
        lines
    }

    fn hints(&self, theme: &Theme) -> Line<'static> {
        let spans = keymap::hint_bindings(&[Scope::Result])
            .into_iter()
            .filter(|binding| binding.action != Action::Retry || self.can_retry())
            .map(|binding| {
                let label = if binding.action == Action::Copy && self.copied {
                    "copied"
                } else {
                    binding.label
                };
                Span::styled(format!("{} {label}  ", binding.keys), theme.muted)
            });
        Line::from(spans.collect::<Vec<_>>())
    }
}

/// Consecutive identical steps as one line with a count: `removing orca ×8`.
fn collapse_repeats(log: &[String]) -> Vec<String> {
    log.chunk_by(|a, b| a == b)
        .map(|run| match run.len() {
            1 => run[0].clone(),
            n => format!("{} ×{n}", run[0]),
        })
        .collect()
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
                self.modal.wheel(other);
                Step::Stay
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame, theme: &Theme) {
        let page = Page {
            panel: Panel::new(self.title.clone()).danger(!self.outcome.is_clean()),
            lines: self.lines(theme),
            footer: self.hints(theme),
        };
        self.modal.render(frame, theme, page);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::key;
    use ratatui::crossterm::event::KeyCode;
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

    #[test]
    fn identical_steps_in_a_row_become_one_line_with_a_count() {
        let log: Vec<String> = ["a", "a", "a", "b", "a"].map(String::from).to_vec();

        let collapsed = collapse_repeats(&log);

        // Assert the run of three folds, and the lone repeats stay apart.
        assert_eq!(collapsed, ["a ×3", "b", "a"]);
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

    #[test]
    fn control_c_quits_without_copying_the_outcome() {
        use crate::tui::widgets::testing::ctrl;
        let mut screen = ResultScreen::new("t", failed());
        screen.copy = |_| panic!("an interrupt must not copy");

        let interrupt = screen.handle(&Event::Key(ctrl('c')));

        // Assert the common interrupt wins over the plain copy binding.
        assert!(matches!(interrupt, Step::Quit));
        assert!(!screen.copied);
    }
}
