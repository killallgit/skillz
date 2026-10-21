//! One list pane's state: a cursor over the rows that pass the pane's filter.

use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::{Position, Rect};

use crate::matcher::fuzzy_rank;
use crate::tui::widgets::input::{InputField, Outcome as Edit};
use crate::tui::widgets::list::ListView;

const PLACEHOLDER: &str = "filter";

#[derive(Debug, Clone)]
pub(super) struct Pane {
    pub list: ListView,
    filter: InputField,
    /// Model row indices that pass the filter, best match first.
    visible: Vec<usize>,
    total: usize,
    /// Where the pane and its rows were last drawn; empty when it was not drawn.
    pub area: Rect,
    pub rows: Rect,
}

impl Pane {
    pub fn new(total: usize) -> Self {
        Pane {
            list: ListView::new(total),
            filter: InputField::new(PLACEHOLDER),
            visible: (0..total).collect(),
            total,
            area: Rect::default(),
            rows: Rect::default(),
        }
    }

    /// The model row under the cursor.
    pub fn row(&self) -> Option<usize> {
        self.list
            .selected()
            .and_then(|at| self.visible.get(at).copied())
    }

    /// Put the cursor on model row `row`, or the nearest row that is shown.
    pub fn select_row(&mut self, row: usize) {
        let nearest = row.min(self.visible.len().saturating_sub(1));
        let at = self.visible.iter().position(|&v| v == row);
        self.list.select(at.unwrap_or(nearest));
    }

    /// The shown-row index under the screen position, if a row is drawn there.
    pub fn shown_at(&self, at: Position) -> Option<usize> {
        if !self.rows.contains(at) {
            return None;
        }
        let shown = self.list.offset() + usize::from(at.y - self.rows.y);
        (shown < self.visible.len()).then_some(shown)
    }

    pub fn visible(&self) -> &[usize] {
        &self.visible
    }

    pub fn total(&self) -> usize {
        self.total
    }

    pub fn query(&self) -> &str {
        self.filter.value()
    }

    pub fn is_filtered(&self) -> bool {
        !self.query().is_empty()
    }

    pub fn field(&self) -> &InputField {
        &self.filter
    }

    /// Feed a key to the filter field. Typing re-filters `texts`; cancelling clears the filter.
    pub fn edit(&mut self, key: KeyEvent, texts: &[String]) -> Edit {
        let before = self.query().to_string();
        let outcome = self.filter.handle(key);
        match outcome {
            Edit::Cancel => self.clear_filter(texts),
            _ if self.query() != before => {
                self.list.select(0);
                self.refilter(texts);
            }
            _ => {}
        }
        outcome
    }

    pub fn clear_filter(&mut self, texts: &[String]) {
        self.filter = InputField::new(PLACEHOLDER);
        self.refilter(texts);
    }

    fn refilter(&mut self, texts: &[String]) {
        self.visible = fuzzy_rank(self.query(), texts);
        self.list.set_len(self.visible.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::{key, keys};
    use ratatui::crossterm::event::KeyCode;

    fn texts() -> Vec<String> {
        ["alpha", "beta", "alps"].map(String::from).to_vec()
    }

    fn type_query(pane: &mut Pane, query: &str) {
        for k in keys(query) {
            pane.edit(k, &texts());
        }
    }

    #[test]
    fn typing_narrows_the_rows_and_keeps_model_indices() {
        let mut pane = Pane::new(3);

        type_query(&mut pane, "alp");

        // "alpha" and "alps" match, in input order; their model rows are 0 and 2.
        assert_eq!(pane.visible(), [0, 2]);
        assert_eq!(pane.list.len(), 2);
        assert_eq!((pane.row(), pane.total()), (Some(0), 3));
    }

    #[test]
    fn the_cursor_row_maps_back_to_the_model_row() {
        let mut pane = Pane::new(3);
        type_query(&mut pane, "alp");

        pane.list.select(1);

        assert_eq!(pane.row(), Some(2));
    }

    #[test]
    fn cancelling_clears_the_filter_and_shows_every_row_again() {
        let mut pane = Pane::new(3);
        type_query(&mut pane, "alp");

        let outcome = pane.edit(key(KeyCode::Esc), &texts());

        assert_eq!(outcome, Edit::Cancel);
        assert!(!pane.is_filtered());
        assert_eq!(pane.visible(), [0, 1, 2]);
    }

    #[test]
    fn submitting_keeps_the_filter() {
        let mut pane = Pane::new(3);
        type_query(&mut pane, "bet");

        let outcome = pane.edit(key(KeyCode::Enter), &texts());

        assert_eq!(outcome, Edit::Submit);
        assert_eq!(pane.visible(), [1]);
    }

    #[test]
    fn select_row_finds_the_row_among_the_shown_ones() {
        let mut pane = Pane::new(3);
        type_query(&mut pane, "alp");

        pane.select_row(2);

        assert_eq!(pane.row(), Some(2));
    }

    #[test]
    fn select_row_past_the_end_lands_on_the_last_row() {
        let mut pane = Pane::new(3);

        pane.select_row(9);

        assert_eq!(pane.row(), Some(2));
    }
}
