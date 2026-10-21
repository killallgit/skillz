//! The tree's data and what is visible: rows in preorder, which are expanded, the filter, and the
//! cursor over the visible rows. No keys and no drawing.

use std::collections::BTreeSet;

use super::menu::Menu;
use crate::matcher::fuzzy_rank;
use crate::tree::{COLUMNS, Node, Widths};
use crate::tui::widgets::input::InputField;
use crate::tui::widgets::list::ListView;

/// One node of the flattened (preorder) tree.
#[derive(Debug, Clone)]
pub(super) struct Row {
    pub label: String,
    pub info: Vec<String>,
    pub cells: [String; COLUMNS.len()],
    pub depth: usize,
    pub parent: Option<usize>,
    pub has_children: bool,
    pub item: Option<usize>,
}

fn flatten(nodes: &[Node], depth: usize, parent: Option<usize>, rows: &mut Vec<Row>) {
    for node in nodes {
        let id = rows.len();
        rows.push(Row {
            label: node.label.clone(),
            info: node.info.clone(),
            cells: node.cells.clone(),
            depth,
            parent,
            has_children: !node.children.is_empty(),
            item: node.item,
        });
        flatten(&node.children, depth + 1, Some(id), rows);
    }
}

/// Which overlay, if any, has the keys.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) enum Overlay {
    #[default]
    None,
    Filter,
    Menu(Menu),
    /// The row whose info modal is open.
    Info(usize),
    Help,
}

/// Collapsible, markable tree. `list` is the cursor over `visible`, which holds row ids in
/// preorder. Groups (top-level rows) and item rows can be marked.
#[derive(Debug, Clone)]
pub struct TreeState {
    pub(super) rows: Vec<Row>,
    pub(super) expanded: BTreeSet<usize>,
    pub(super) marks: BTreeSet<usize>,
    pub(super) list: ListView,
    pub(super) filter: InputField,
    pub(super) visible: Vec<usize>,
    pub(super) overlay: Overlay,
    /// A picker: Enter accepts the marks directly instead of opening the action menu.
    pub(super) pick: bool,
    pub(super) widths: Widths,
}

impl TreeState {
    pub fn new(nodes: &[Node]) -> Self {
        let mut rows = Vec::new();
        flatten(nodes, 0, None, &mut rows);
        let mut state = TreeState {
            rows,
            expanded: BTreeSet::new(),
            marks: BTreeSet::new(),
            list: ListView::new(0),
            filter: InputField::new("filter"),
            visible: vec![],
            overlay: Overlay::None,
            pick: false,
            widths: Widths::of(nodes, 2),
        };
        state.refresh();
        state
    }

    /// The same tree as a picker: Enter accepts the marked rows (or the row under the cursor).
    pub fn picker(nodes: &[Node]) -> Self {
        TreeState {
            pick: true,
            ..Self::new(nodes)
        }
    }

    pub(super) fn current(&self) -> Option<usize> {
        self.list
            .selected()
            .and_then(|at| self.visible.get(at).copied())
    }

    pub(super) fn is_open(&self, id: usize) -> bool {
        !self.filter.value().is_empty() || self.expanded.contains(&id)
    }

    pub(super) fn toggle_open(&mut self, id: usize) {
        if !self.expanded.remove(&id) {
            self.expanded.insert(id);
        }
    }

    pub(super) fn select_row(&mut self, id: usize) {
        let at = self.visible.iter().position(|&v| v == id).unwrap_or(0);
        self.list.select(at);
    }

    pub(super) fn clear_filter(&mut self) {
        self.filter = InputField::new("filter");
        self.refresh();
    }

    pub(super) fn refresh(&mut self) {
        self.visible = if self.filter.value().is_empty() {
            self.expanded_rows()
        } else {
            self.matching_rows()
        };
        self.list.set_len(self.visible.len());
    }

    fn expanded_rows(&self) -> Vec<usize> {
        let mut shown = vec![false; self.rows.len()];
        for (i, row) in self.rows.iter().enumerate() {
            shown[i] = row
                .parent
                .is_none_or(|p| shown[p] && self.expanded.contains(&p));
        }
        (0..self.rows.len()).filter(|&i| shown[i]).collect()
    }

    /// Fuzzy matches plus every ancestor, so a match always shows where it lives.
    fn matching_rows(&self) -> Vec<usize> {
        let labels: Vec<String> = self.rows.iter().map(|r| r.label.clone()).collect();
        let mut keep = BTreeSet::new();
        for mut id in fuzzy_rank(self.filter.value(), &labels) {
            while keep.insert(id) {
                match self.rows[id].parent {
                    Some(p) => id = p,
                    None => break,
                }
            }
        }
        keep.into_iter().collect()
    }
}
