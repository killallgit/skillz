//! A tab bar. `1`-`9` jump to a tab, `[` and `]` step through them.

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::Outcome;
use crate::tui::theme::Theme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tabs {
    titles: Vec<String>,
    selected: usize,
}

impl Tabs {
    pub fn new(titles: &[&str]) -> Self {
        Tabs {
            titles: titles.iter().map(|t| t.to_string()).collect(),
            selected: 0,
        }
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn handle(&mut self, key: KeyEvent) -> Outcome {
        let target = match key.code {
            KeyCode::Char(c @ '1'..='9') => Some(c as usize - '1' as usize),
            KeyCode::Char(']') => Some(self.next()),
            KeyCode::Char('[') => Some(self.previous()),
            _ => None,
        };
        match target {
            Some(t) if t < self.titles.len() && t != self.selected => {
                self.selected = t;
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn next(&self) -> usize {
        (self.selected + 1) % self.titles.len().max(1)
    }

    fn previous(&self) -> usize {
        (self.selected + self.titles.len().max(1) - 1) % self.titles.len().max(1)
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        frame.render_widget(Paragraph::new(self.line(theme)), area);
    }

    fn line(&self, theme: &Theme) -> Line<'static> {
        let mut spans = Vec::new();
        for (i, title) in self.titles.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" │ ", theme.border));
            }
            spans.push(self.tab(i, title, theme));
        }
        Line::from(spans)
    }

    fn tab(&self, index: usize, title: &str, theme: &Theme) -> Span<'static> {
        let label = format!("{} {title}", index + 1);
        if index == self.selected {
            Span::styled(label, theme.accent.add_modifier(Modifier::BOLD))
        } else {
            Span::styled(label, theme.muted)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::{key, render_to_buffer};

    fn tabs() -> Tabs {
        Tabs::new(&["alpha", "beta", "gamma"])
    }

    #[test]
    fn number_keys_select_that_tab() {
        let mut t = tabs();
        assert_eq!(t.handle(key(KeyCode::Char('3'))), Outcome::Changed);
        assert_eq!(t.selected(), 2);
    }

    #[test]
    fn out_of_range_number_is_ignored() {
        let mut t = tabs();
        assert_eq!(t.handle(key(KeyCode::Char('7'))), Outcome::Ignored);
        assert_eq!(t.selected(), 0);
    }

    #[test]
    fn reselecting_the_current_tab_changes_nothing() {
        assert_eq!(tabs().handle(key(KeyCode::Char('1'))), Outcome::Ignored);
    }

    #[test]
    fn right_bracket_steps_forward_and_wraps() {
        let mut t = tabs();
        for _ in 0..3 {
            t.handle(key(KeyCode::Char(']')));
        }
        assert_eq!(t.selected(), 0);
    }

    #[test]
    fn left_bracket_wraps_backwards_from_the_first_tab() {
        let mut t = tabs();
        t.handle(key(KeyCode::Char('[')));
        assert_eq!(t.selected(), 2);
    }

    #[test]
    fn brackets_do_nothing_with_one_tab() {
        let mut t = Tabs::new(&["only"]);
        assert_eq!(t.handle(key(KeyCode::Char(']'))), Outcome::Ignored);
    }

    #[test]
    fn selected_tab_is_accented_and_others_muted() {
        let theme = Theme::dark();
        let t = tabs();
        let buffer = render_to_buffer(40, 1, |f| t.render(f, f.area(), &theme));
        assert_eq!(buffer[(0, 0)].fg, theme.accent.fg.unwrap());
        let other = "1 alpha │ ".chars().count() as u16;
        assert_eq!(buffer[(other, 0)].fg, theme.muted.fg.unwrap());
    }
}
