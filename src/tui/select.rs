use std::collections::BTreeSet;

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::{FilterKey, Mark, draw_list, event_loop, filter_key, is_ctrl_c, step_cursor};
use crate::matcher::fuzzy_rank;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Continue,
    Accept(Vec<usize>),
    /// `i` on the option at this index.
    Info(usize),
    Cancel,
}

/// Multi-select list state. `cursor` indexes `visible`, which holds indices into `options`.
#[derive(Debug, Clone)]
pub struct SelectState {
    options: Vec<String>,
    cursor: usize,
    selected: BTreeSet<usize>,
    filter: String,
    filtering: bool,
    visible: Vec<usize>,
}

impl SelectState {
    pub fn new(options: &[String]) -> Self {
        SelectState {
            visible: (0..options.len()).collect(),
            options: options.to_vec(),
            cursor: 0,
            selected: BTreeSet::new(),
            filter: String::new(),
            filtering: false,
        }
    }

    pub fn handle(&mut self, key: KeyEvent) -> Outcome {
        if is_ctrl_c(key) {
            return Outcome::Cancel;
        }
        if self.filtering {
            match filter_key(&mut self.filter, key.code) {
                FilterKey::Edited => {
                    self.cursor = 0;
                    self.refilter();
                }
                FilterKey::Left => {
                    self.filtering = false;
                    self.refilter();
                }
                FilterKey::Other => self.navigate(key.code),
            }
            return Outcome::Continue;
        }
        match key.code {
            KeyCode::Char(' ') => self.toggle(),
            KeyCode::Char('a') => self.toggle_all(),
            KeyCode::Char('/') => self.filtering = true,
            KeyCode::Char('i') => return self.current().map_or(Outcome::Continue, Outcome::Info),
            KeyCode::Enter => return Outcome::Accept(self.accepted()),
            KeyCode::Esc | KeyCode::Char('q') => return Outcome::Cancel,
            code => self.navigate(code),
        }
        Outcome::Continue
    }

    fn navigate(&mut self, code: KeyCode) {
        let up = match code {
            KeyCode::Up | KeyCode::Char('k') => true,
            KeyCode::Down | KeyCode::Char('j') => false,
            _ => return,
        };
        self.cursor = step_cursor(self.cursor, self.visible.len(), up);
    }

    fn current(&self) -> Option<usize> {
        self.visible.get(self.cursor).copied()
    }

    fn toggle(&mut self) {
        if let Some(i) = self.current()
            && !self.selected.remove(&i)
        {
            self.selected.insert(i);
        }
    }

    fn toggle_all(&mut self) {
        if self.visible.iter().all(|i| self.selected.contains(i)) {
            self.visible.iter().for_each(|i| {
                self.selected.remove(i);
            });
        } else {
            self.selected.extend(&self.visible);
        }
    }

    fn accepted(&self) -> Vec<usize> {
        if self.selected.is_empty() {
            self.current().into_iter().collect()
        } else {
            self.selected.iter().copied().collect()
        }
    }

    fn refilter(&mut self) {
        self.visible = fuzzy_rank(&self.filter, &self.options);
        self.cursor = self.cursor.min(self.visible.len().saturating_sub(1));
    }

    fn draw(&self, title: &str, frame: &mut Frame) {
        let rows = self
            .visible
            .iter()
            .map(|&i| {
                let mark = if self.selected.contains(&i) {
                    Mark::All
                } else {
                    Mark::No
                };
                format!("{}{}", mark.column(), self.options[i])
            })
            .collect();
        let footer = if self.filtering {
            format!("/{}", self.filter)
        } else {
            format!(
                "↑↓/jk move · space mark · a all · i info · / filter · enter accept · esc/q cancel  {}",
                self.filter
            )
        };
        draw_list(frame, title, rows, self.cursor, footer);
    }
}

/// Run the interactive multi-select list. Returns chosen indices into `options` (ascending);
/// empty when cancelled.
pub fn run(title: &str, options: &[String]) -> anyhow::Result<Vec<usize>> {
    event_loop(
        &mut SelectState::new(options),
        |s, f| s.draw(title, f),
        |s, k| match s.handle(k) {
            // The info modal (skillz-atj) takes over `Info`.
            Outcome::Continue | Outcome::Info(_) => None,
            Outcome::Accept(v) => Some(v),
            Outcome::Cancel => Some(vec![]),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::{key, keys};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::KeyModifiers;

    fn state(opts: &[&str]) -> SelectState {
        SelectState::new(&opts.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    fn feed(s: &mut SelectState, ks: impl IntoIterator<Item = KeyEvent>) -> Outcome {
        ks.into_iter()
            .map(|k| s.handle(k))
            .find(|o| *o != Outcome::Continue)
            .unwrap_or(Outcome::Continue)
    }

    #[test]
    fn enter_without_selection_accepts_cursor_item() {
        let mut s = state(&["a", "b", "c"]);
        assert_eq!(feed(&mut s, keys("jj")), Outcome::Continue);
        assert_eq!(s.cursor, 2);
        assert_eq!(feed(&mut s, keys("j")), Outcome::Continue);
        assert_eq!(s.cursor, 2);
        assert_eq!(s.handle(key(KeyCode::Enter)), Outcome::Accept(vec![2]));
    }

    #[test]
    fn toggles_return_ascending_selection() {
        let mut s = state(&["a", "b", "c"]);
        let seq = [
            key(KeyCode::Down),
            key(KeyCode::Down),
            key(KeyCode::Char(' ')),
            key(KeyCode::Up),
            key(KeyCode::Up),
            key(KeyCode::Char(' ')),
            key(KeyCode::Char('k')),
            key(KeyCode::Enter),
        ];
        assert_eq!(feed(&mut s, seq), Outcome::Accept(vec![0, 2]));
    }

    #[test]
    fn space_twice_unselects() {
        let mut s = state(&["a", "b"]);
        feed(&mut s, keys("  "));
        assert!(s.selected.is_empty());
    }

    #[test]
    fn toggle_all_selects_then_clears_visible() {
        let mut s = state(&["a", "b", "c"]);
        feed(&mut s, keys("a"));
        assert_eq!(s.selected.len(), 3);
        feed(&mut s, keys("a"));
        assert!(s.selected.is_empty());
    }

    #[test]
    fn filter_narrows_and_letters_are_not_commands() {
        let mut s = state(&["git-commit", "tdd", "caveman-commit"]);
        feed(&mut s, keys("/tdq"));
        assert!(s.filtering);
        assert_eq!(s.filter, "tdq");
        s.handle(key(KeyCode::Backspace));
        assert_eq!(s.visible, vec![1]);
        assert!(s.selected.is_empty());
        s.handle(key(KeyCode::Enter));
        assert!(!s.filtering);
        assert_eq!(feed(&mut s, keys("a")), Outcome::Continue);
        assert_eq!(s.selected, BTreeSet::from([1]));
    }

    #[test]
    fn toggle_all_only_touches_visible() {
        let mut s = state(&["git-commit", "tdd", "caveman-commit"]);
        feed(&mut s, keys("/commit"));
        s.handle(key(KeyCode::Down));
        s.handle(key(KeyCode::Enter));
        feed(&mut s, keys("a"));
        assert_eq!(s.selected, BTreeSet::from([0, 2]));
    }

    #[test]
    fn leaving_filter_keeps_cursor_item() {
        let mut s = state(&["git-commit", "tdd", "caveman-commit"]);
        feed(&mut s, keys("/commit"));
        s.handle(key(KeyCode::Down));
        s.handle(key(KeyCode::Enter));
        assert_eq!(s.handle(key(KeyCode::Enter)), Outcome::Accept(vec![2]));
    }

    #[test]
    fn esc_in_filter_clears_it_then_esc_cancels() {
        let mut s = state(&["a", "b"]);
        feed(&mut s, keys("/zzz"));
        assert!(s.visible.is_empty());
        assert_eq!(s.handle(key(KeyCode::Enter)), Outcome::Continue);
        assert_eq!(feed(&mut s, keys("/")), Outcome::Continue);
        s.handle(key(KeyCode::Esc));
        assert!(!s.filtering && s.filter.is_empty());
        assert_eq!(s.visible, vec![0, 1]);
        assert_eq!(s.handle(key(KeyCode::Esc)), Outcome::Cancel);
    }

    #[test]
    fn enter_with_nothing_visible_accepts_nothing() {
        let mut s = state(&["a"]);
        feed(&mut s, keys("/zzz"));
        s.handle(key(KeyCode::Enter));
        feed(&mut s, keys(" j"));
        assert_eq!(s.handle(key(KeyCode::Enter)), Outcome::Accept(vec![]));
    }

    #[test]
    fn q_and_ctrl_c_cancel() {
        assert_eq!(
            state(&["a"]).handle(key(KeyCode::Char('q'))),
            Outcome::Cancel
        );
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let mut s = state(&["a"]);
        feed(&mut s, keys("/"));
        assert_eq!(s.handle(ctrl_c), Outcome::Cancel);
    }

    #[test]
    fn renders_without_panicking() {
        let mut s = state(&["a", "b"]);
        let mut term = Terminal::new(TestBackend::new(40, 6)).unwrap();
        term.draw(|f| s.draw("t", f)).unwrap();
        feed(&mut s, keys(" /a"));
        term.draw(|f| s.draw("t", f)).unwrap();
    }

    #[test]
    fn i_reports_the_option_under_the_cursor_even_when_filtered() {
        let mut s = state(&["a", "b", "c"]);
        feed(&mut s, keys("j"));
        assert_eq!(s.handle(key(KeyCode::Char('i'))), Outcome::Info(1));
        feed(&mut s, keys("/c"));
        feed(&mut s, [key(KeyCode::Enter)]);
        assert_eq!(s.handle(key(KeyCode::Char('i'))), Outcome::Info(2));
    }
}
