//! A row of labelled buttons, one of them focused. Every confirm dialog draws its answers with
//! this, the cancelling one first.

use ratatui::text::{Line, Span};

use crate::tui::theme::Theme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Buttons {
    labels: Vec<String>,
    focus: usize,
}

impl Buttons {
    /// `labels` left to right, with `focus` on the one at that position.
    pub fn new(labels: &[&str], focus: usize) -> Self {
        Buttons {
            labels: labels.iter().map(|l| l.to_string()).collect(),
            focus: focus.min(labels.len().saturating_sub(1)),
        }
    }

    pub fn focus(&self) -> usize {
        self.focus
    }

    pub fn set_label(&mut self, at: usize, label: &str) {
        if let Some(slot) = self.labels.get_mut(at) {
            *slot = label.to_string();
        }
    }

    pub fn next(&mut self) {
        self.focus = (self.focus + 1) % self.labels.len().max(1);
    }

    pub fn previous(&mut self) {
        let count = self.labels.len().max(1);
        self.focus = (self.focus + count - 1) % count;
    }

    /// `[ Cancel ]   [ Delete ]`, the focused one reversed.
    pub fn line(&self, theme: &Theme) -> Line<'static> {
        let mut spans = Vec::new();
        for (at, label) in self.labels.iter().enumerate() {
            if at > 0 {
                spans.push(Span::raw("   "));
            }
            let style = if at == self.focus {
                theme.choice
            } else {
                theme.text
            };
            spans.push(Span::styled(format!("[ {label} ]"), style));
        }
        Line::from(spans)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_wraps_both_ways() {
        let mut buttons = Buttons::new(&["Cancel", "Delete"], 0);

        buttons.previous();
        let wrapped_back = buttons.focus();
        buttons.next();

        assert_eq!((wrapped_back, buttons.focus()), (1, 0));
    }

    #[test]
    fn only_the_focused_button_is_reversed() {
        let theme = Theme::dark();
        let line = Buttons::new(&["Cancel", "Delete"], 1).line(&theme);

        let styles: Vec<_> = line
            .spans
            .iter()
            .filter(|s| s.content.starts_with('['))
            .map(|s| s.style)
            .collect();

        assert_eq!(styles, [theme.text, theme.choice]);
    }

    #[test]
    fn a_label_can_be_replaced() {
        let mut buttons = Buttons::new(&["Cancel", "Apply"], 1);

        buttons.set_label(1, "Dry run");

        assert!(buttons.line(&Theme::dark()).to_string().contains("Dry run"));
    }
}
