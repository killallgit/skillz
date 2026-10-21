//! The detail pane: the selected row's lines, styled by role, wrapped and scrollable.

use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use super::model::{DetailLine, EntryKind, Focus};
use crate::tui::theme::Theme;
use crate::tui::widgets::panel::Panel;
use crate::tui::widgets::scroll::Scroll;

const CHILD_INDENT: &str = "      ";

/// Where the pane scrolled to, and for which row; a different row starts at the top again.
#[derive(Debug, Default)]
pub(super) struct Detail {
    scroll: Scroll,
    row: Option<(Focus, usize)>,
    area: Rect,
    rows: usize,
}

impl Detail {
    pub fn draw(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        row: Option<(Focus, usize)>,
        (lines, theme): (&[DetailLine], &Theme),
    ) {
        if self.row != row {
            self.row = row;
            self.scroll = Scroll::default();
        }
        let inner = Panel::new("Detail").render(frame, area, theme);
        let styled: Vec<Line<'static>> = lines.iter().map(|l| styled_line(l, theme)).collect();
        self.area = area;
        self.rows = wrapped_rows(&styled, inner.width);
        let page = Paragraph::new(styled)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll.offset(), 0));
        frame.render_widget(page, inner);
    }

    pub fn forget_area(&mut self) {
        self.area = Rect::default();
    }

    pub fn contains(&self, at: Position) -> bool {
        self.area.contains(at)
    }

    /// Scroll for the wheel; true when the event was one.
    pub fn wheel(&mut self, event: &Event) -> bool {
        self.scroll.wheel(event, self.rows)
    }

    pub fn page(&mut self, down: bool) {
        self.scroll.page(down, self.rows);
    }
}

/// The rows `lines` take at `width` columns, with a spare row per wrapped line because words
/// that do not fit move whole to the next one.
fn wrapped_rows(lines: &[Line], width: u16) -> usize {
    let width = usize::from(width.max(1));
    lines
        .iter()
        .map(|line| {
            let columns = line.width();
            columns.div_ceil(width).max(1) + usize::from(columns > width)
        })
        .sum()
}

fn kind_style(kind: EntryKind, theme: &Theme) -> Style {
    match kind {
        EntryKind::Skill => theme.accent,
        EntryKind::Plugin => theme.ok,
        EntryKind::Subagent => theme.warn,
        EntryKind::Hook | EntryKind::Mcp => theme.title,
    }
}

fn styled_line(line: &DetailLine, theme: &Theme) -> Line<'static> {
    match line {
        DetailLine::Blank => Line::default(),
        DetailLine::Heading(text) => Line::styled(text.clone(), theme.title),
        DetailLine::Text(text) => Line::raw(text.clone()),
        DetailLine::Entry(kind, text) => Line::from(vec![
            Span::styled(format!("{:<7}", kind.tag()), kind_style(*kind, theme)),
            Span::raw(text.clone()),
        ]),
        DetailLine::Child(text) => Line::styled(format!("{CHILD_INDENT}{text}"), theme.muted),
        DetailLine::Status(state, text) => Line::styled(text.clone(), theme.state_style(*state)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::theme::State;

    #[test]
    fn an_entry_line_starts_with_its_kind_tag_in_the_kinds_colour() {
        let theme = Theme::dark();

        let line = styled_line(&DetailLine::Entry(EntryKind::Plugin, "p".into()), &theme);

        // The tag is the first span and carries the plugin colour; the text follows plain.
        assert_eq!(line.spans[0].style, theme.ok);
        assert_eq!(line.spans[1].content, "p");
    }

    #[test]
    fn status_lines_use_their_states_colour() {
        let theme = Theme::dark();

        let line = styled_line(&DetailLine::Status(State::Warn, "x".into()), &theme);

        assert_eq!(line.style, theme.warn);
    }

    #[test]
    fn long_lines_count_every_row_they_wrap_to() {
        let lines = [Line::raw("a".repeat(25)), Line::raw("b")];

        let rows = wrapped_rows(&lines, 10);

        // 25 columns at width 10 is 3 rows plus a spare; the short line is one.
        assert_eq!(rows, 4 + 1);
    }

    #[test]
    fn a_different_row_scrolls_back_to_the_top() {
        let theme = Theme::dark();
        let lines = vec![DetailLine::Text("x".into()); 30];
        let mut detail = Detail::default();
        let draw = |detail: &mut Detail, row| {
            crate::tui::widgets::testing::render_to_buffer(20, 6, |f| {
                detail.draw(f, f.area(), row, (&lines, &theme));
            });
        };
        draw(&mut detail, Some((Focus::Projects, 0)));
        detail.page(true);
        let scrolled = detail.scroll.offset();

        draw(&mut detail, Some((Focus::Projects, 1)));

        // Page down moved it; selecting another row put it back.
        assert_eq!((scrolled > 0, detail.scroll.offset()), (true, 0));
    }
}
