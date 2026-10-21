//! One-line key hints. Hints are given most important first; when the width is short the
//! last ones are dropped, and `? help` always stays at the end.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

use crate::tui::theme::Theme;

const SEPARATOR: &str = "  ";
const HELP: (&str, &str) = ("?", "help");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBar {
    hints: Vec<(String, String)>,
}

impl KeyBar {
    pub fn new(hints: &[(&str, &str)]) -> Self {
        KeyBar {
            hints: hints
                .iter()
                .map(|(k, l)| (k.to_string(), l.to_string()))
                .collect(),
        }
    }

    /// The hints that fit in `width` columns, help included, in display order.
    pub fn fit(&self, width: u16) -> Vec<(&str, &str)> {
        let all: Vec<(&str, &str)> = self
            .hints
            .iter()
            .map(|(k, l)| (k.as_str(), l.as_str()))
            .collect();
        (0..=all.len())
            .rev()
            .map(|keep| with_help(&all[..keep]))
            .find(|shown| total_width(shown) <= width as usize)
            .unwrap_or_else(|| vec![HELP])
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let line = hint_line(&self.fit(area.width), theme);
        frame.render_widget(Paragraph::new(line), area);
    }
}

fn with_help<'a>(hints: &[(&'a str, &'a str)]) -> Vec<(&'a str, &'a str)> {
    let mut shown: Vec<_> = hints.iter().copied().filter(|h| *h != HELP).collect();
    shown.push(HELP);
    shown
}

fn hint_width(hint: &(&str, &str)) -> usize {
    hint.0.width() + 1 + hint.1.width()
}

fn total_width(hints: &[(&str, &str)]) -> usize {
    let separators = SEPARATOR.width() * hints.len().saturating_sub(1);
    hints.iter().map(hint_width).sum::<usize>() + separators
}

fn hint_line<'a>(hints: &[(&'a str, &'a str)], theme: &Theme) -> Line<'a> {
    let mut spans = Vec::new();
    for (i, (key, label)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(SEPARATOR));
        }
        spans.push(Span::styled(*key, theme.accent));
        spans.push(Span::styled(format!(" {label}"), theme.muted));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::render_to_buffer;

    fn bar() -> KeyBar {
        KeyBar::new(&[("enter", "open"), ("x", "mark"), ("/", "filter")])
    }

    #[test]
    fn everything_shows_when_there_is_room() {
        assert_eq!(bar().fit(80).len(), 4);
    }

    #[test]
    fn help_is_always_last() {
        for width in [80, 30, 10, 3] {
            assert_eq!(bar().fit(width).last(), Some(&HELP));
        }
    }

    #[test]
    fn lowest_priority_hints_drop_first() {
        let bar = bar();
        let shown = bar.fit(26);
        assert_eq!(shown, vec![("enter", "open"), ("x", "mark"), HELP]);
    }

    #[test]
    fn highest_priority_hint_outlasts_the_rest() {
        let bar = bar();
        let shown = bar.fit(18);
        assert_eq!(shown.first(), Some(&("enter", "open")));
        assert_eq!(shown.len(), 2);
    }

    #[test]
    fn an_explicit_help_hint_is_not_duplicated() {
        let bar = KeyBar::new(&[("?", "help"), ("q", "quit")]);
        assert_eq!(bar.fit(80).iter().filter(|h| **h == HELP).count(), 1);
    }

    #[test]
    fn nothing_fits_but_help_remains() {
        assert_eq!(bar().fit(0), vec![HELP]);
    }

    #[test]
    fn keys_are_accent_and_labels_are_muted() {
        let theme = Theme::dark();
        let buffer = render_to_buffer(40, 1, |f| bar().render(f, f.area(), &theme));
        assert_eq!(buffer[(0, 0)].fg, theme.accent.fg.unwrap());
        assert_eq!(buffer[(6, 0)].fg, theme.muted.fg.unwrap());
    }
}
