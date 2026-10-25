//! A collapsible outline: rows in preorder with parents, which ones are open, a fuzzy filter
//! and a scrolling cursor over what is visible. It knows labels and shape, not what a row
//! means or how it is drawn.

mod keys;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;

use super::input::InputField;
use super::list::ListView;
use crate::app::matcher::{fuzzy_rank, match_positions};
use crate::tui::filter;
use crate::tui::glyphs::Glyphs;
use crate::tui::theme::Theme;

const FILTER_PLACEHOLDER: &str = "filter";

/// One row of an outline's shape: its parent's id and the text the filter matches.
pub type Shape = (Option<usize>, String);

#[derive(Debug, Clone)]
pub struct Outline {
    parents: Vec<Option<usize>>,
    labels: Vec<String>,
    has_children: Vec<bool>,
    expanded: BTreeSet<usize>,
    filter: InputField,
    visible: Vec<usize>,
    list: ListView,
}

impl Outline {
    /// Every row closed; parents come before their children.
    pub fn new(shape: Vec<Shape>) -> Self {
        let (parents, labels): (Vec<_>, Vec<_>) = shape.into_iter().unzip();
        let mut has_children = vec![false; parents.len()];
        parents
            .iter()
            .flatten()
            .for_each(|&p| has_children[p] = true);
        let mut outline = Outline {
            parents,
            labels,
            has_children,
            expanded: BTreeSet::new(),
            filter: InputField::new(FILTER_PLACEHOLDER),
            visible: vec![],
            list: ListView::new(0),
        };
        outline.refresh();
        outline
    }

    /// The same outline over a new shape. A row that was open stays open when a row with the same
    /// label is still there; every other row with children starts closed. The filter and the
    /// cursor stay where they were.
    pub fn reshape(&mut self, shape: Vec<Shape>) {
        let open: Vec<String> = self
            .expanded
            .iter()
            .map(|&id| self.labels[id].clone())
            .collect();
        let at = self.list.selected().unwrap_or(0);
        let filter = self.filter.clone();
        *self = Outline::new(shape);
        self.expanded = (0..self.labels.len())
            .filter(|&id| self.has_children[id] && open.contains(&self.labels[id]))
            .collect();
        self.filter = filter;
        self.refresh();
        self.list.select(at);
    }

    /// Row ids in display order.
    pub fn visible(&self) -> &[usize] {
        &self.visible
    }

    /// The row under the cursor.
    pub fn current(&self) -> Option<usize> {
        self.list
            .selected()
            .and_then(|at| self.visible.get(at).copied())
    }

    #[cfg(test)]
    pub fn selected(&self) -> Option<usize> {
        self.list.selected()
    }

    pub fn label(&self, id: usize) -> &str {
        &self.labels[id]
    }

    pub fn parent(&self, id: usize) -> Option<usize> {
        self.parents[id]
    }

    pub fn depth(&self, id: usize) -> usize {
        std::iter::successors(self.parents[id], |&p| self.parents[p]).count()
    }

    pub fn is_open(&self, id: usize) -> bool {
        self.is_filtered() || self.expanded.contains(&id)
    }

    pub fn toggle_open(&mut self, id: usize) {
        if !self.expanded.remove(&id) {
            self.expanded.insert(id);
        }
        self.refresh();
    }

    pub fn select_row(&mut self, id: usize) {
        let at = self.visible.iter().position(|&v| v == id).unwrap_or(0);
        self.list.select(at);
    }

    pub fn is_filtered(&self) -> bool {
        !self.filter.value().is_empty()
    }

    pub fn query(&self) -> &str {
        self.filter.value()
    }

    pub fn field(&self) -> &InputField {
        &self.filter
    }

    pub fn clear_filter(&mut self) {
        self.filter = InputField::new(FILTER_PLACEHOLDER);
        self.refresh();
    }

    /// Where the filter matched in row `id`'s label, for highlighting.
    pub fn matched(&self, id: usize) -> Vec<usize> {
        match_positions(self.query(), &self.labels[id])
    }

    pub fn open_all(&mut self) {
        self.expanded = (0..self.labels.len())
            .filter(|&id| self.has_children[id])
            .collect();
        self.refresh();
    }

    /// How many rows sit directly under row `id`.
    pub fn child_count(&self, id: usize) -> usize {
        self.parents.iter().filter(|&&p| p == Some(id)).count()
    }

    /// `▾` for an open row with children, `▸` for a closed one, a space for a leaf.
    pub fn caret(&self, id: usize, glyphs: &Glyphs) -> &'static str {
        match (self.has_children[id], self.is_open(id)) {
            (false, _) => " ",
            (true, true) => glyphs.open,
            (true, false) => glyphs.closed,
        }
    }

    /// `n/shown`, and with a filter how many rows it hides.
    pub fn badge(&self) -> String {
        filter::badge(
            self.list.selected(),
            self.visible.len(),
            self.labels.len(),
            self.is_filtered(),
        )
    }

    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        lines: &[Line<'static>],
        theme: &Theme,
    ) {
        self.list.render(frame, area, lines, theme);
    }

    fn refresh(&mut self) {
        self.visible = if self.is_filtered() {
            self.matching()
        } else {
            self.open_rows()
        };
        self.list.set_len(self.visible.len());
    }

    fn open_rows(&self) -> Vec<usize> {
        let mut shown = vec![false; self.labels.len()];
        for (id, parent) in self.parents.iter().enumerate() {
            shown[id] = parent.is_none_or(|p| shown[p] && self.expanded.contains(&p));
        }
        (0..self.labels.len()).filter(|&id| shown[id]).collect()
    }

    /// Fuzzy matches plus every ancestor, so a match always shows where it lives.
    fn matching(&self) -> Vec<usize> {
        let mut keep = BTreeSet::new();
        for mut id in fuzzy_rank(self.query(), &self.labels) {
            while keep.insert(id) {
                match self.parents[id] {
                    Some(parent) => id = parent,
                    None => break,
                }
            }
        }
        keep.into_iter().collect()
    }
}
