//! The `?` overlay: the keymap's bindings for a screen, grouped, in a list that scrolls.

use ratatui::Frame;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::keymap::{self, Action, Binding, Scope};
use super::theme::Theme;
use super::widgets::list::ListView;
use super::widgets::modal::{self, ModalSize};
use super::widgets::panel::Panel;

const WIDTH: u16 = 52;

/// Whether the overlay keeps the keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Open,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Help {
    scopes: Vec<Scope>,
    list: ListView,
}

impl Help {
    pub fn new(scopes: &[Scope]) -> Self {
        let rows = rows(scopes).len();
        Help {
            scopes: scopes.to_vec(),
            list: ListView::new(rows),
        }
    }

    pub fn handle(&mut self, key: ratatui::crossterm::event::KeyEvent) -> Verdict {
        match keymap::resolve(Scope::Common, key) {
            Some(Action::Back | Action::Help) => Verdict::Closed,
            Some(action) => {
                self.list.apply(action);
                Verdict::Open
            }
            None => Verdict::Open,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, theme: &Theme) {
        let lines = lines(&self.scopes, theme);
        let size = ModalSize {
            width: WIDTH,
            height: lines.len() as u16 + 2,
        };
        let inner = modal::open(frame, size, Panel::new("Keys"), theme);
        self.list.render(frame, inner, &lines, theme);
    }
}

enum Row {
    Section(Scope),
    Key(&'static Binding),
}

fn rows(scopes: &[Scope]) -> Vec<Row> {
    keymap::help_sections(scopes)
        .into_iter()
        .flat_map(|(scope, bindings)| {
            let keys = bindings.into_iter().map(Row::Key);
            std::iter::once(Row::Section(scope)).chain(keys)
        })
        .collect()
}

fn lines(scopes: &[Scope], theme: &Theme) -> Vec<Line<'static>> {
    let rows = rows(scopes);
    let width = rows
        .iter()
        .filter_map(|r| match r {
            Row::Key(b) => Some(b.keys.width()),
            Row::Section(_) => None,
        })
        .max()
        .unwrap_or(0);
    rows.iter()
        .map(|row| match row {
            Row::Section(scope) => Line::styled(scope.title(), theme.title),
            Row::Key(b) => Line::from(vec![
                Span::styled(format!("{:<width$}  ", b.keys), theme.accent),
                Span::styled(b.label, theme.text),
            ]),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::{key, render_to_buffer};
    use ratatui::crossterm::event::KeyCode;

    #[test]
    fn every_listed_binding_gets_a_row_under_a_section_per_group() {
        let sections = keymap::help_sections(&[Scope::Plugins]);

        let rows = rows(&[Scope::Plugins]);

        let listed: usize = sections.iter().map(|(_, b)| b.len()).sum();
        assert_eq!(rows.len(), listed + sections.len());
        assert!(matches!(rows[0], Row::Section(Scope::Plugins)));
    }

    #[test]
    fn labels_start_in_one_column() {
        let theme = Theme::dark();

        let lines = lines(&[Scope::Tree], &theme);

        // Each key row is a padded key span, then the label; the key spans share a width.
        let widths: Vec<usize> = lines
            .iter()
            .filter(|l| l.spans.len() == 2)
            .map(|l| l.spans[0].content.width())
            .collect();
        assert!(widths.windows(2).all(|w| w[0] == w[1]));
        assert!(!widths.is_empty());
    }

    #[test]
    fn escape_and_question_mark_close_while_navigation_keeps_it_open() {
        let mut help = Help::new(&[Scope::Plugins]);

        let moved = help.handle(key(KeyCode::Down));
        let ignored = help.handle(key(KeyCode::Char('z')));
        let esc = help.handle(key(KeyCode::Esc));
        let again = help.handle(key(KeyCode::Char('?')));
        let quit_letter = help.handle(key(KeyCode::Char('q')));

        assert_eq!(
            (moved, ignored, esc, again, quit_letter),
            (
                Verdict::Open,
                Verdict::Open,
                Verdict::Closed,
                Verdict::Closed,
                Verdict::Open
            )
        );
    }

    #[test]
    fn a_short_screen_scrolls_the_overlay_instead_of_cutting_it() {
        let theme = Theme::dark();
        let mut help = Help::new(&[Scope::Plugins]);
        let top = render_to_buffer(60, 10, |f| help.render(f, &theme));
        for _ in 0..30 {
            help.handle(key(KeyCode::Down));
        }

        let scrolled = render_to_buffer(60, 10, |f| help.render(f, &theme));

        // The window moved: the overlay shows different rows than at the top.
        assert_ne!(top, scrolled);
    }
}
