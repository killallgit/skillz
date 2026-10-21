//! A scrolling list that keeps its cursor in view: gutter marker, scrollbar thumb, and
//! `↑ n more` / `↓ n more` hints when rows are cut off.

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::Outcome;
use crate::tui::theme::Theme;

const DEFAULT_PAGE: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListView {
    len: usize,
    selected: usize,
    offset: usize,
    page: usize,
}

impl ListView {
    pub fn new(len: usize) -> Self {
        ListView {
            len,
            selected: 0,
            offset: 0,
            page: DEFAULT_PAGE,
        }
    }

    pub fn selected(&self) -> Option<usize> {
        (self.len > 0).then_some(self.selected)
    }

    /// Move the cursor to `index`, clamped to the last row.
    pub fn select(&mut self, index: usize) {
        self.selected = index.min(self.len.saturating_sub(1));
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Replace the row count, pulling the cursor back inside the new bounds.
    pub fn set_len(&mut self, len: usize) {
        self.len = len;
        self.selected = self.selected.min(len.saturating_sub(1));
        self.offset = self.offset.min(self.selected);
    }

    pub fn handle(&mut self, key: KeyEvent) -> Outcome {
        let before = self.selected;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => self.move_down(1),
            KeyCode::PageUp => self.selected = self.selected.saturating_sub(self.page),
            KeyCode::PageDown => self.move_down(self.page),
            KeyCode::Home => self.selected = 0,
            KeyCode::End => self.selected = self.len.saturating_sub(1),
            _ => return Outcome::Ignored,
        }
        if self.selected == before {
            Outcome::Ignored
        } else {
            Outcome::Changed
        }
    }

    fn move_down(&mut self, by: usize) {
        self.selected = (self.selected + by).min(self.len.saturating_sub(1));
    }

    /// Scroll the minimum needed so the cursor row is inside a window of `height` rows.
    pub fn keep_visible(&mut self, height: usize) {
        self.page = height.max(1);
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + self.page {
            self.offset = self.selected + 1 - self.page;
        }
        self.offset = self.offset.min(self.len.saturating_sub(self.page));
    }

    pub fn hidden_above(&self) -> usize {
        self.offset
    }

    pub fn hidden_below(&self) -> usize {
        self.len.saturating_sub(self.offset + self.page)
    }

    /// Draw `rows` (one line per item) inside `area`, scrolled to the cursor.
    pub fn render(&mut self, frame: &mut Frame, area: Rect, rows: &[Line<'static>], theme: &Theme) {
        self.keep_visible(area.height as usize);
        let scrolls = self.len > self.page;
        let content = Rect {
            width: area.width.saturating_sub(u16::from(scrolls)),
            ..area
        };
        let window = rows.iter().enumerate().skip(self.offset).take(self.page);
        let lines: Vec<Line> = window
            .map(|(i, row)| self.decorate(i, row, theme))
            .collect();
        frame.render_widget(Paragraph::new(lines), content);
        self.highlight_cursor_row(frame, content, theme);
        if scrolls {
            self.render_scrollbar(frame, area, theme);
            self.render_hints(frame, content, theme);
        }
    }

    fn decorate(&self, index: usize, row: &Line<'static>, theme: &Theme) -> Line<'static> {
        let current = index == self.selected;
        let gutter = if current {
            Span::styled("▌ ", theme.accent)
        } else {
            Span::raw("  ")
        };
        let mut spans = vec![gutter];
        spans.extend(row.spans.iter().cloned());
        let line = Line::from(spans);
        if current {
            line.style(theme.selection)
        } else {
            line
        }
    }

    fn highlight_cursor_row(&self, frame: &mut Frame, content: Rect, theme: &Theme) {
        let row = self.selected.saturating_sub(self.offset) as u16;
        if self.is_empty() || row >= content.height {
            return;
        }
        let area = Rect::new(content.x, content.y + row, content.width, 1);
        frame.buffer_mut().set_style(area, theme.selection);
    }

    fn render_scrollbar(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let (start, size) = thumb(self.len, area.height as usize, self.offset);
        let cells: Vec<Line> = (0..area.height as usize)
            .map(|y| match (start..start + size).contains(&y) {
                true => Line::from(Span::styled("█", theme.accent)),
                false => Line::from(Span::styled("│", theme.border)),
            })
            .collect();
        let track = Rect::new(area.right() - 1, area.y, 1, area.height);
        frame.render_widget(Paragraph::new(cells), track);
    }

    fn render_hints(&self, frame: &mut Frame, content: Rect, theme: &Theme) {
        if self.hidden_above() > 0 {
            let text = format!("↑ {} more", self.hidden_above());
            hint(frame, content, content.y, &text, theme);
        }
        if self.hidden_below() > 0 {
            let text = format!("↓ {} more", self.hidden_below());
            hint(frame, content, content.bottom() - 1, &text, theme);
        }
    }
}

fn hint(frame: &mut Frame, content: Rect, y: u16, text: &str, theme: &Theme) {
    let width = (text.chars().count() as u16 + 2).min(content.width);
    let area = Rect::new(content.right() - width, y, width, 1);
    let line = Line::from(Span::styled(format!(" {text}"), theme.muted)).right_aligned();
    frame.render_widget(Paragraph::new(line), area);
}

/// Thumb start and size, in track cells, for a window of `height` rows at `offset` of `len`.
pub fn thumb(len: usize, height: usize, offset: usize) -> (usize, usize) {
    if len <= height || height == 0 {
        return (0, height);
    }
    let size = (height * height / len).max(1);
    let travel = height - size;
    let start = (offset * travel).div_ceil(len - height);
    (start.min(travel), size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::{key, render_to_buffer, row_text};

    fn rows(n: usize) -> Vec<Line<'static>> {
        (0..n).map(|i| Line::from(format!("row{i}"))).collect()
    }

    fn view_at(len: usize, selected: usize, height: usize) -> ListView {
        let mut view = ListView::new(len);
        view.selected = selected;
        view.keep_visible(height);
        view
    }

    #[test]
    fn cursor_below_the_window_scrolls_it_down() {
        assert_eq!(view_at(20, 7, 5).offset(), 3);
    }

    #[test]
    fn cursor_above_the_window_scrolls_it_up() {
        let mut view = view_at(20, 7, 5);
        view.selected = 1;
        view.keep_visible(5);
        assert_eq!(view.offset(), 1);
    }

    #[test]
    fn cursor_inside_the_window_leaves_offset_alone() {
        assert_eq!(view_at(20, 2, 5).offset(), 0);
    }

    #[test]
    fn down_stops_at_the_last_row() {
        let mut view = view_at(3, 2, 5);
        assert_eq!(view.handle(key(KeyCode::Down)), Outcome::Ignored);
        assert_eq!(view.selected(), Some(2));
    }

    #[test]
    fn up_stops_at_the_first_row() {
        let mut view = ListView::new(3);
        assert_eq!(view.handle(key(KeyCode::Up)), Outcome::Ignored);
    }

    #[test]
    fn page_down_moves_by_the_viewport() {
        let mut view = view_at(30, 0, 8);
        view.handle(key(KeyCode::PageDown));
        assert_eq!(view.selected(), Some(8));
    }

    #[test]
    fn end_jumps_to_the_last_row() {
        let mut view = ListView::new(30);
        assert_eq!(view.handle(key(KeyCode::End)), Outcome::Changed);
        assert_eq!(view.selected(), Some(29));
    }

    #[test]
    fn select_clamps_to_the_last_row() {
        let mut list = ListView::new(3);
        list.select(9);
        assert_eq!(list.selected(), Some(2));
    }

    #[test]
    fn empty_list_has_no_selection() {
        assert_eq!(ListView::new(0).selected(), None);
    }

    #[test]
    fn shrinking_clamps_the_cursor() {
        let mut view = view_at(10, 9, 5);
        view.set_len(4);
        assert_eq!(view.selected(), Some(3));
    }

    #[test]
    fn hidden_counts_split_around_the_window() {
        let view = view_at(20, 7, 5);
        assert_eq!((view.hidden_above(), view.hidden_below()), (3, 12));
    }

    #[test]
    fn thumb_is_whole_track_when_everything_fits() {
        assert_eq!(thumb(4, 5, 0), (0, 5));
    }

    #[test]
    fn thumb_reaches_the_bottom_at_the_last_offset() {
        let (start, size) = thumb(20, 5, 15);
        assert_eq!(start + size, 5);
    }

    #[test]
    fn thumb_starts_at_the_top_at_offset_zero() {
        assert_eq!(thumb(20, 5, 0).0, 0);
    }

    #[test]
    fn selected_row_is_marked_in_the_gutter_and_highlighted() {
        let theme = Theme::dark();
        let mut view = ListView::new(3);
        let buffer = render_to_buffer(12, 3, |f| view.render(f, f.area(), &rows(3), &theme));
        assert_eq!(buffer[(0, 0)].symbol(), "▌");
        assert_eq!(buffer[(0, 1)].symbol(), " ");
        assert_eq!(buffer[(11, 0)].bg, theme.selection.bg.unwrap());
        assert_ne!(buffer[(11, 1)].bg, theme.selection.bg.unwrap());
    }

    #[test]
    fn overflowing_list_draws_a_scrollbar_thumb_in_the_last_column() {
        let theme = Theme::dark();
        let mut view = ListView::new(20);
        let buffer = render_to_buffer(14, 5, |f| view.render(f, f.area(), &rows(20), &theme));
        assert_eq!(buffer[(13, 0)].symbol(), "█");
        assert_eq!(buffer[(13, 4)].symbol(), "│");
    }

    #[test]
    fn fitting_list_draws_no_scrollbar() {
        let theme = Theme::dark();
        let mut view = ListView::new(3);
        let buffer = render_to_buffer(14, 5, |f| view.render(f, f.area(), &rows(3), &theme));
        assert!(!row_text(&buffer, 0).contains('█'));
    }

    #[test]
    fn overflow_hint_counts_hidden_rows_below() {
        let theme = Theme::dark();
        let mut view = ListView::new(20);
        let buffer = render_to_buffer(20, 5, |f| view.render(f, f.area(), &rows(20), &theme));
        assert!(row_text(&buffer, 4).contains("15"));
        assert!(!row_text(&buffer, 0).contains('↑'));
    }
}
