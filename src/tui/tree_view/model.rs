//! The tree's data: what each row says and which are marked, over a shared outline that holds
//! the shape, the open rows, the filter and the cursor. No keys and no drawing.

use std::collections::BTreeSet;

use super::menu::Menu;
use crate::tree::{COLUMNS, Node, Widths};
use crate::tui::help::Help;
use crate::tui::widgets::outline::{Outline, Shape};

/// What a row says besides its label, which the outline keeps.
#[derive(Debug, Clone)]
pub(super) struct Row {
    pub info: Vec<String>,
    pub cells: [String; COLUMNS.len()],
    pub item: Option<usize>,
}

/// Rows in preorder, with the outline's shape alongside.
fn flatten(nodes: &[Node], parent: Option<usize>, rows: &mut Vec<Row>, shape: &mut Vec<Shape>) {
    for node in nodes {
        let id = rows.len();
        rows.push(Row {
            info: node.info.clone(),
            cells: node.cells.clone(),
            item: node.item,
        });
        shape.push((parent, node.label.clone()));
        flatten(&node.children, Some(id), rows, shape);
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
    Help(Help),
}

/// Collapsible, markable tree. Groups (top-level rows) and item rows can be marked.
#[derive(Debug, Clone)]
pub struct TreeState {
    pub(super) rows: Vec<Row>,
    pub(super) outline: Outline,
    pub(super) marks: BTreeSet<usize>,
    pub(super) overlay: Overlay,
    /// A picker: Enter accepts the marks directly instead of opening the action menu.
    pub(super) pick: bool,
    pub(super) widths: Widths,
}

impl TreeState {
    pub fn new(nodes: &[Node]) -> Self {
        let (mut rows, mut shape) = (Vec::new(), Vec::new());
        flatten(nodes, None, &mut rows, &mut shape);
        TreeState {
            rows,
            outline: Outline::new(shape),
            marks: BTreeSet::new(),
            overlay: Overlay::None,
            pick: false,
            widths: Widths::of(nodes, 2),
        }
    }

    /// The same tree as a picker: Enter accepts the marked rows (or the row under the cursor).
    pub fn picker(nodes: &[Node]) -> Self {
        TreeState {
            pick: true,
            ..Self::new(nodes)
        }
    }
}
