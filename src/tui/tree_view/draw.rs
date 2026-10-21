//! Drawing the tree: a focused panel around the list, a key bar or filter line, and overlays.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::marks::Mark;
use super::model::{Overlay, Row, TreeState};
use crate::tui::help;
use crate::tui::theme::Theme;
use crate::tui::widgets::keybar::KeyBar;
use crate::tui::widgets::modal::{self, ModalSize};
use crate::tui::widgets::panel::Panel;

/// Two columns: an `X` (or `-` for part) and a space, so rows line up.
fn mark_span(mark: Mark, theme: &Theme) -> Span<'static> {
    match mark {
        Mark::No => Span::raw("  "),
        Mark::Part => Span::styled("- ", theme.warn),
        Mark::All => Span::styled("X ", theme.ok),
    }
}

/// Room taken before a row's text: the list's cursor gutter and the mark column.
const TEXT_INDENT: usize = 4;

impl TreeState {
    pub fn draw(&mut self, title: &str, frame: &mut Frame, theme: &Theme) {
        let [body, foot] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(frame.area());
        self.draw_list(title, frame, body, theme);
        self.draw_footer(frame, foot, theme);
        self.draw_overlay(frame, theme);
    }

    fn hints(&self) -> [(&'static str, &'static str); 7] {
        let enter = if self.pick { "confirm" } else { "actions" };
        [
            ("↑↓", "move"),
            ("space", "expand"),
            ("x", "mark"),
            ("i", "info"),
            ("enter", enter),
            ("/", "filter"),
            ("q", "quit"),
        ]
    }

    fn panel_title(&self, title: &str) -> String {
        match self.filter.value() {
            "" => title.to_string(),
            filter => format!("{title} /{filter}"),
        }
    }

    fn draw_list(&mut self, title: &str, frame: &mut Frame, area: Rect, theme: &Theme) {
        let panel = Panel::new(self.panel_title(title))
            .focused(true)
            .position_badge(self.list.selected(), self.visible.len());
        let inner = panel.render(frame, area, theme);
        let [cols, rows] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(inner);
        let header = format!("{}{}", " ".repeat(TEXT_INDENT), self.widths.header());
        frame.render_widget(Paragraph::new(header).style(theme.muted), cols);
        let lines: Vec<Line<'static>> = self
            .visible
            .iter()
            .map(|&id| self.row_line(id, theme))
            .collect();
        self.list.render(frame, rows, &lines, theme);
    }

    fn row_line(&self, id: usize, theme: &Theme) -> Line<'static> {
        let row = &self.rows[id];
        let caret = match (row.has_children, self.is_open(id)) {
            (false, _) => " ",
            (true, true) => "▾",
            (true, false) => "▸",
        };
        let prefix = format!("{}{caret} ", "  ".repeat(row.depth));
        let text = self
            .widths
            .line(&prefix, &row.label, &row.cells, row.item.is_none());
        Line::from(vec![
            mark_span(self.mark(id), theme),
            Span::styled(text, row_style(row, theme)),
        ])
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if self.overlay != Overlay::Filter {
            return KeyBar::new(&self.hints()).render(frame, area, theme);
        }
        let [slash, field] =
            Layout::horizontal([Constraint::Length(1), Constraint::Min(1)]).areas(area);
        frame.render_widget(Paragraph::new("/").style(theme.accent), slash);
        self.filter.render(frame, field, theme);
    }

    fn draw_overlay(&mut self, frame: &mut Frame, theme: &Theme) {
        let hints = self.hints();
        match &mut self.overlay {
            Overlay::Menu(menu) => menu.render(frame, theme),
            Overlay::Info(id) => draw_info(frame, &self.rows[*id], theme),
            Overlay::Help => help::draw(frame, "Keys", &hints, theme),
            Overlay::None | Overlay::Filter => {}
        }
    }
}

fn row_style(row: &Row, theme: &Theme) -> ratatui::style::Style {
    match (row.item, row.depth) {
        (None, 0) => theme.title,
        (None, _) => theme.muted,
        _ => theme.text,
    }
}

fn draw_info(frame: &mut Frame, row: &Row, theme: &Theme) {
    let size = ModalSize {
        width: frame.area().width * 4 / 5,
        height: row.info.len() as u16 + 2,
    };
    let inner = modal::open(frame, size, Panel::new(row.label.clone()), theme);
    frame.render_widget(Paragraph::new(row.info.join("\n")).style(theme.text), inner);
}
