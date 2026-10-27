//! A tab bar: which tab is selected, and stepping through them with wrap-around.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

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

    /// Jump to tab `index`; out of range is ignored.
    pub fn select(&mut self, index: usize) {
        if index < self.titles.len() {
            self.selected = index;
        }
    }

    pub fn next(&mut self) {
        self.selected = (self.selected + 1) % self.titles.len().max(1);
    }

    pub fn previous(&mut self) {
        let count = self.titles.len().max(1);
        self.selected = (self.selected + count - 1) % count;
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
        let label = title.to_string();
        if index == self.selected {
            Span::styled(label, theme.tab_active)
        } else {
            Span::styled(label, theme.muted)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::render_to_buffer;

    fn tabs() -> Tabs {
        Tabs::new(&["alpha", "beta", "gamma"])
    }

    #[test]
    fn select_jumps_and_ignores_out_of_range() {
        let mut t = tabs();

        t.select(2);
        t.select(9);

        assert_eq!(t.selected(), 2);
    }

    #[test]
    fn next_steps_forward_and_wraps() {
        let mut t = tabs();

        let visited: Vec<usize> = (0..3)
            .map(|_| {
                t.next();
                t.selected()
            })
            .collect();

        assert_eq!(visited, [1, 2, 0]);
    }

    #[test]
    fn previous_wraps_backwards_from_the_first_tab() {
        let mut t = tabs();

        t.previous();

        assert_eq!(t.selected(), 2);
    }

    #[test]
    fn stepping_with_one_tab_stays_put() {
        let mut t = Tabs::new(&["only"]);

        t.next();
        t.previous();

        assert_eq!(t.selected(), 0);
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
