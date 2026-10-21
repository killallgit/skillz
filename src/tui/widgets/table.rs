//! Aligned columns: fixed or flexible widths, ellipsis truncation by display width.
//! Produces `Line`s, so rows can be drawn directly or fed to a `ListView`.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::tui::theme::Theme;

const GAP: usize = 1;
const ELLIPSIS: char = '…';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    Fixed(u16),
    /// Share of the space left after fixed columns, by weight.
    Flex(u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub header: String,
    pub width: Width,
}

impl Column {
    pub fn new(header: impl Into<String>, width: Width) -> Self {
        Column {
            header: header.into(),
            width,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    columns: Vec<Column>,
}

impl Table {
    pub fn new(columns: Vec<Column>) -> Self {
        Table { columns }
    }

    /// Resolved column widths for a table `total` cells wide.
    pub fn widths(&self, total: u16) -> Vec<usize> {
        let gaps = GAP * self.columns.len().saturating_sub(1);
        let mut room = (total as usize).saturating_sub(gaps);
        let mut widths: Vec<usize> = self
            .columns
            .iter()
            .map(|c| fixed_width(c, &mut room))
            .collect();
        share_flex(&self.columns, &mut widths, room);
        widths
    }

    pub fn header_line(&self, total: u16, theme: &Theme) -> Line<'static> {
        let headers: Vec<&str> = self.columns.iter().map(|c| c.header.as_str()).collect();
        self.line(&headers, total, theme.muted)
    }

    pub fn row_line<S: AsRef<str>>(&self, cells: &[S], total: u16, theme: &Theme) -> Line<'static> {
        let cells: Vec<&str> = cells.iter().map(AsRef::as_ref).collect();
        self.line(&cells, total, theme.text)
    }

    /// One row as plain text, padded and truncated to the column widths.
    pub fn row_text<S: AsRef<str>>(&self, cells: &[S], total: u16) -> String {
        let widths = self.widths(total);
        let padded: Vec<String> = widths
            .iter()
            .enumerate()
            .map(|(i, w)| pad(&truncate(cells.get(i).map_or("", AsRef::as_ref), *w), *w))
            .collect();
        padded.join(&" ".repeat(GAP))
    }

    fn line(&self, cells: &[&str], total: u16, style: ratatui::style::Style) -> Line<'static> {
        Line::from(Span::styled(self.row_text(cells, total), style))
    }

    /// Draw the header on the first row and `rows` beneath it, as many as fit.
    pub fn render<S: AsRef<str>>(
        &self,
        frame: &mut Frame,
        area: Rect,
        rows: &[Vec<S>],
        theme: &Theme,
    ) {
        let [head, body] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
        frame.render_widget(Paragraph::new(self.header_line(area.width, theme)), head);
        let lines: Vec<Line> = rows
            .iter()
            .map(|r| self.row_line(r, area.width, theme))
            .collect();
        frame.render_widget(Paragraph::new(lines), body);
    }
}

fn fixed_width(column: &Column, room: &mut usize) -> usize {
    match column.width {
        Width::Fixed(w) => {
            let taken = (w as usize).min(*room);
            *room -= taken;
            taken
        }
        Width::Flex(_) => 0,
    }
}

/// Split `room` among flex columns by weight; leftover cells go to the earliest columns.
fn share_flex(columns: &[Column], widths: &mut [usize], room: usize) {
    let weights: Vec<usize> = columns.iter().map(flex_weight).collect();
    let total: usize = weights.iter().sum();
    if total == 0 {
        return;
    }
    let mut left = room;
    for (width, weight) in widths.iter_mut().zip(&weights) {
        *width += room * weight / total;
        left -= room * weight / total;
    }
    for (width, weight) in widths.iter_mut().zip(&weights) {
        if left > 0 && *weight > 0 {
            *width += 1;
            left -= 1;
        }
    }
}

fn flex_weight(column: &Column) -> usize {
    match column.width {
        Width::Flex(w) => w as usize,
        Width::Fixed(_) => 0,
    }
}

/// `text` cut to at most `width` display cells, ending in an ellipsis when something was lost.
pub fn truncate(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_string();
    }
    let budget = width.saturating_sub(ELLIPSIS.width().unwrap_or(1));
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        let w = c.width().unwrap_or(0);
        if used + w > budget {
            break;
        }
        out.push(c);
        used += w;
    }
    if width > 0 {
        out.push(ELLIPSIS);
    }
    out
}

fn pad(text: &str, width: usize) -> String {
    let missing = width.saturating_sub(text.width());
    format!("{text}{}", " ".repeat(missing))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::{render_to_buffer, row_text};

    fn table() -> Table {
        Table::new(vec![
            Column::new("name", Width::Flex(1)),
            Column::new("kind", Width::Fixed(6)),
            Column::new("where", Width::Flex(1)),
        ])
    }

    #[test]
    fn widths_fill_the_total_exactly() {
        let widths = table().widths(40);
        assert_eq!(widths.iter().sum::<usize>() + 2, 40);
    }

    #[test]
    fn fixed_columns_keep_their_width() {
        assert_eq!(table().widths(40)[1], 6);
    }

    #[test]
    fn flex_columns_split_by_weight() {
        let t = Table::new(vec![
            Column::new("a", Width::Flex(3)),
            Column::new("b", Width::Flex(1)),
        ]);
        assert_eq!(t.widths(41), vec![30, 10]);
    }

    #[test]
    fn fixed_columns_shrink_when_the_table_is_too_narrow() {
        let widths = table().widths(5);
        assert!(widths.iter().sum::<usize>() <= 5);
    }

    #[test]
    fn text_that_fits_is_untouched() {
        assert_eq!(truncate("abc", 3), "abc");
    }

    #[test]
    fn long_text_ends_in_an_ellipsis_within_the_width() {
        let cut = truncate("abcdefgh", 5);
        assert_eq!(cut.width(), 5);
        assert!(cut.ends_with(ELLIPSIS));
    }

    #[test]
    fn wide_characters_are_never_split() {
        let cut = truncate("日本語日本語", 6);
        assert!(cut.width() <= 6);
        assert!(cut.ends_with(ELLIPSIS));
    }

    #[test]
    fn zero_width_yields_nothing() {
        assert_eq!(truncate("abc", 0), "");
    }

    #[test]
    fn row_lines_have_the_requested_display_width() {
        let theme = Theme::dark();
        let line = table().row_line(&["日本語", "k", "somewhere long enough to cut"], 30, &theme);
        assert_eq!(line.width(), 30);
    }

    #[test]
    fn missing_cells_render_blank() {
        let theme = Theme::dark();
        let line = table().row_line(&["only"], 30, &theme);
        assert_eq!(line.width(), 30);
    }

    #[test]
    fn header_is_drawn_muted_above_the_rows() {
        let theme = Theme::dark();
        let rows = vec![vec!["a", "b", "c"]];
        let buffer = render_to_buffer(30, 3, |f| table().render(f, f.area(), &rows, &theme));
        assert_eq!(buffer[(0, 0)].fg, theme.muted.fg.unwrap());
        assert_eq!(buffer[(0, 1)].symbol(), "a");
        assert!(row_text(&buffer, 2).trim().is_empty());
    }
}
