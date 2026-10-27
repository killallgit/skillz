//! How a switch ended: a line for each item that changed, then anything that went wrong and the
//! steps that ran. It is answered with an OK button, plus Copy and Retry when something failed.

use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyEvent};
use ratatui::text::{Line, Span};

use super::app::{Screen, Step};
use super::clipboard;
use super::keymap::{self, Action, Scope};
use super::result_items::done_lines;
use super::theme::Theme;
use super::widgets::buttons::Buttons;
use super::widgets::panel::Panel;
use super::widgets::text_modal::{Page, TextModal};
use crate::app::review::{Next, Outcome};

/// What a button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pressed {
    Copy,
    Retry,
    Ok,
}

pub struct ResultScreen {
    title: String,
    outcome: Outcome,
    modal: TextModal,
    copied: bool,
    copy: fn(&str),
    /// The buttons' meanings, in the order of `buttons`; OK is last and focused.
    choices: Vec<Pressed>,
    buttons: Buttons,
}

impl ResultScreen {
    pub fn new(title: &str, outcome: Outcome) -> Self {
        let mut choices = Vec::new();
        if !outcome.is_clean() {
            choices.push(Pressed::Copy);
        }
        if !outcome.failures.is_empty() {
            choices.push(Pressed::Retry);
        }
        choices.push(Pressed::Ok);
        let labels: Vec<&str> = choices
            .iter()
            .map(|choice| match choice {
                Pressed::Copy => "Copy",
                Pressed::Retry => "Retry",
                Pressed::Ok => "OK",
            })
            .collect();
        ResultScreen {
            title: title.to_string(),
            outcome,
            modal: TextModal::default(),
            copied: false,
            copy: copy_to_terminal,
            buttons: Buttons::new(&labels, choices.len() - 1),
            choices,
        }
    }

    fn press(&mut self, choice: Pressed) -> Step<Next> {
        match choice {
            Pressed::Copy => {
                (self.copy)(&self.clip());
                self.copied = true;
                if let Some(at) = self.choices.iter().position(|c| *c == Pressed::Copy) {
                    self.buttons.set_label(at, "Copied");
                }
                Step::Stay
            }
            Pressed::Retry => Step::Act(Next::Retry),
            Pressed::Ok => Step::Act(Next::Done),
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
            Some(Action::Copy) if self.choices.contains(&Pressed::Copy) => {
                return self.press(Pressed::Copy);
            }
            Some(Action::Retry) if self.can_retry() => return Step::Act(Next::Retry),
            Some(Action::Select) => return self.press(self.choices[self.buttons.focus()]),
            Some(Action::Back | Action::Quit) => return Step::Act(Next::Done),
            Some(Action::Left) => self.buttons.previous(),
            Some(Action::Right) => self.buttons.next(),
            Some(Action::Interrupt) => return Step::Quit,
            Some(movement) => self.modal.navigate(movement),
            None => {}
        }
        Step::Stay
    }

    fn lines(&self, theme: &Theme) -> Vec<Line<'static>> {
        let mut lines = done_lines(&self.outcome, theme);
        if lines.is_empty() && self.outcome.is_clean() {
            lines.push(Line::styled("Nothing changed", theme.muted));
        }
        lines.extend(problem_lines(&self.outcome, theme));
        let steps = collapse_repeats(&self.outcome.log);
        if !self.outcome.is_clean() && !steps.is_empty() {
            lines.push(Line::default());
            lines.push(Line::styled("steps", theme.muted));
            lines.extend(steps.iter().map(|l| Line::raw(format!("  {l}"))));
        }
        lines
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
            buttons: Some(self.buttons.line(theme)),
            help: None,
            reveal: None,
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

        // Line 0 is the blank that opens the problems; the failure follows it.
        assert_eq!(lines[1].spans[0].style, theme.danger);
    }

    fn drawn_text(screen: &mut ResultScreen) -> String {
        use crate::tui::widgets::testing::{render_to_buffer, row_text};
        let theme = Theme::dark();
        let buffer = render_to_buffer(100, 30, |f| screen.draw(f, &theme));
        (0..30)
            .map(|y| row_text(&buffer, y))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn a_clean_result_is_the_list_of_what_changed_and_one_ok_button() {
        let outcome = Outcome {
            items: vec![crate::app::review::ItemEvent {
                target: crate::app::review::Target::Remove("a".into()),
                label: "killall-skills".into(),
                phase: crate::app::review::Phase::Succeeded,
            }],
            ..Outcome::default()
        };
        let mut screen = ResultScreen::new("Change", outcome);

        let text = drawn_text(&mut screen);

        assert!(text.contains("Removed killall-skills") && text.contains("[ OK ]"));
        assert!(!text.contains("Copy") && !text.contains("Retry"));
        // No counts and no key hints.
        assert!(
            !text.contains("installed 0") && !text.contains("enter") && !text.contains("close")
        );
    }

    #[test]
    fn a_failed_result_offers_copy_and_retry_before_ok_and_enter_answers_ok() {
        let mut screen = ResultScreen::new("Change", failed());

        let text = drawn_text(&mut screen);
        let enter = answer(&mut screen, KeyCode::Enter);

        let (copy, retry, ok) = (
            text.find("[ Copy ]").unwrap(),
            text.find("[ Retry ]").unwrap(),
            text.find("[ OK ]").unwrap(),
        );
        assert!(copy < retry && retry < ok);
        assert_eq!(enter, Some(Next::Done));
    }

    #[test]
    fn moving_to_retry_and_pressing_enter_retries_and_copy_says_copied() {
        let mut screen = ResultScreen::new("Change", failed());
        screen.copy = |_| {};

        answer(&mut screen, KeyCode::Left);
        let retry = answer(&mut screen, KeyCode::Enter);
        answer(&mut screen, KeyCode::Left);
        let copy = answer(&mut screen, KeyCode::Enter);

        assert_eq!((retry, copy), (Some(Next::Retry), None));
        assert!(drawn_text(&mut screen).contains("[ Copied ]"));
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
