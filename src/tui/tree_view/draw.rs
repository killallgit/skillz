//! Drawing the tree: a focused panel around the list, a key bar or filter line, and overlays.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::marks::Mark;
use super::model::{Overlay, Row, TreeState};
use crate::tui::filter;
use crate::tui::keymap::{self, Scope};
use crate::tui::theme::Theme;
use crate::tui::widgets::empty;
use crate::tui::widgets::keybar::KeyBar;
use crate::tui::widgets::margin;
use crate::tui::widgets::modal::{self, ModalSize};
use crate::tui::widgets::panel::Panel;

/// Room taken before a row's text: the list's cursor gutter and the mark column.
const TEXT_INDENT: usize = 4;

/// One glyph and a space, so rows line up.
fn mark_span(mark: Mark, theme: &Theme) -> Span<'static> {
    let glyphs = &theme.glyphs;
    match mark {
        Mark::No => Span::styled(format!("{} ", glyphs.unchecked), theme.muted),
        Mark::Part => Span::styled(format!("{} ", glyphs.partial), theme.warn),
        Mark::All => Span::styled(format!("{} ", glyphs.checked), theme.ok),
    }
}

impl TreeState {
    pub fn draw(&mut self, title: &str, frame: &mut Frame, theme: &Theme) {
        let [body, foot] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)])
            .areas(margin::inset(frame.area()));
        self.draw_list(title, frame, body, theme);
        self.draw_footer(frame, foot, theme);
        self.draw_overlay(frame, theme);
    }

    fn hints(&self) -> Vec<(&'static str, &'static str)> {
        let mut hints = keymap::hints(&[Scope::Tree]);
        if self.pick {
            hints
                .iter_mut()
                .filter(|h| h.0 == "enter")
                .for_each(|h| h.1 = "confirm");
        }
        hints
    }

    fn badge(&self) -> String {
        let counts = self.outline.badge();
        match self.marked_items() {
            0 => counts,
            marked => format!("{marked} marked  {counts}"),
        }
    }

    fn draw_list(&mut self, title: &str, frame: &mut Frame, area: Rect, theme: &Theme) {
        let panel = Panel::new(filter::title(title, self.outline.query()))
            .focused(true)
            .bare()
            .badge(self.badge());
        let inner = panel.render(frame, area, theme).inner(Margin::new(1, 0));
        if self.outline.visible().is_empty() {
            return empty::render(frame, inner, self.empty_message(theme));
        }
        let [cols, rows] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(inner);
        let header = format!("{}{}", " ".repeat(TEXT_INDENT), self.widths.header());
        frame.render_widget(Paragraph::new(header).style(theme.muted), cols);
        let lines: Vec<Line<'static>> = self
            .outline
            .visible()
            .iter()
            .map(|&id| self.row_line(id, theme))
            .collect();
        self.outline.render(frame, rows, &lines, theme);
    }

    fn empty_message(&self, theme: &Theme) -> Vec<Line<'static>> {
        if self.outline.is_filtered() {
            return vec![
                Line::styled("Nothing matches", theme.muted),
                Line::styled("esc clears the filter", theme.muted),
            ];
        }
        vec![Line::styled("Nothing to show", theme.muted)]
    }

    fn row_line(&self, id: usize, theme: &Theme) -> Line<'static> {
        let row = &self.rows[id];
        let depth = self.outline.depth(id);
        let caret = self.outline.caret(id, &theme.glyphs);
        let prefix = format!("{}{caret} ", "  ".repeat(depth));
        let label = self.outline.label(id);
        let text = self
            .widths
            .line(&prefix, label, &row.cells, row.item.is_none());
        let positions = self.outline.matched(id);
        let offset = prefix.chars().count();
        let mut spans = vec![mark_span(self.mark(id), theme)];
        spans.extend(filter::highlight(
            &text,
            &positions,
            offset,
            row_style(row, depth, theme),
            theme,
        ));
        Line::from(spans)
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if self.overlay == Overlay::Filter {
            return filter::draw_prompt(frame, area, self.outline.field(), theme);
        }
        KeyBar::new(&self.hints()).render(frame, area, theme);
    }

    fn draw_overlay(&mut self, frame: &mut Frame, theme: &Theme) {
        match &mut self.overlay {
            Overlay::Menu(menu) => menu.render(frame, theme),
            Overlay::Info(id) => {
                draw_info(frame, (self.outline.label(*id), &self.rows[*id]), theme)
            }
            Overlay::Help(help) => help.render(frame, theme),
            Overlay::None | Overlay::Filter => {}
        }
    }
}

fn row_style(row: &Row, depth: usize, theme: &Theme) -> Style {
    match (row.item, depth) {
        (None, 0) => theme.title,
        (None, _) => theme.muted,
        _ => theme.text,
    }
}

fn draw_info(frame: &mut Frame, (label, row): (&str, &Row), theme: &Theme) {
    let size = ModalSize {
        width: frame.area().width * 4 / 5,
        height: row.info.len() as u16 + 2,
    };
    let inner = modal::open(frame, size, Panel::new(label), theme);
    frame.render_widget(Paragraph::new(row.info.join("\n")).style(theme.text), inner);
}
