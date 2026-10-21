//! The `?` overlay: every key hint of a screen, one per line.

use ratatui::Frame;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

use super::theme::Theme;
use super::widgets::modal::{self, ModalSize};
use super::widgets::panel::Panel;

pub type Hints = [(&'static str, &'static str)];

pub fn draw(frame: &mut Frame, title: &str, hints: &Hints, theme: &Theme) {
    let size = ModalSize {
        width: 44,
        height: hints.len() as u16 + 2,
    };
    let inner = modal::open(frame, size, Panel::new(title), theme);
    frame.render_widget(Paragraph::new(lines(hints, theme)), inner);
}

fn lines(hints: &Hints, theme: &Theme) -> Vec<Line<'static>> {
    let key_width = hints.iter().map(|(k, _)| k.width()).max().unwrap_or(0);
    hints
        .iter()
        .map(|(key, label)| {
            Line::from(vec![
                Span::styled(format!("{key:<key_width$}  "), theme.accent),
                Span::styled(*label, theme.text),
            ])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::{render_to_buffer, row_text};

    const HINTS: &Hints = &[("x", "mark"), ("tab", "pane")];

    #[test]
    fn each_hint_gets_its_own_row_with_the_key_column_padded() {
        let theme = Theme::dark();

        let buffer = render_to_buffer(60, 10, |f| draw(f, "help", HINTS, &theme));

        // Labels start in the same column, so the shorter key is padded to the longer one.
        let rows: Vec<String> = (0..10).map(|y| row_text(&buffer, y)).collect();
        let column = |label: &str| {
            rows.iter()
                .find_map(|r| r.find(label).map(|b| r[..b].chars().count()))
        };
        assert_eq!(column("mark"), column("pane"));
        assert!(column("mark").is_some());
    }
}
