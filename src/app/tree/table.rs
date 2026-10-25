//! Aligned text for a node tree: column widths, one row of text, and the plain-output table.

use super::{COLUMNS, Node};

/// Column widths for a set of rows: the name column (indent included) and each cell column.
/// A column no row fills has width 0 and is left out of the header and the rows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Widths {
    name: usize,
    cells: [usize; COLUMNS.len()],
}

impl Widths {
    /// `extra` columns are reserved before each name (the terminal list's expand caret).
    pub fn of(nodes: &[Node], extra: usize) -> Self {
        let mut w = Widths::default();
        w.add(nodes, 0, extra);
        for (width, title) in w.cells.iter_mut().zip(COLUMNS) {
            if *width > 0 {
                *width = (*width).max(title.len());
            }
        }
        w
    }

    fn add(&mut self, nodes: &[Node], depth: usize, extra: usize) {
        for n in nodes {
            // Detail leaves have no cells and may be long, so they never set a width.
            if n.cells.iter().any(|c| !c.is_empty()) || n.item.is_some() {
                self.name = self.name.max(2 * depth + extra + n.label.chars().count());
                for (w, c) in self.cells.iter_mut().zip(&n.cells) {
                    *w = (*w).max(c.chars().count());
                }
            }
            self.add(&n.children, depth + 1, extra);
        }
    }

    pub fn header(&self) -> String {
        self.join("NAME", &COLUMNS.map(String::from))
    }

    /// One row: `prefix` (indent, caret) and `node`'s name padded to the name column, then the
    /// cells. A detail leaf is just its text.
    pub fn row(&self, prefix: &str, node: &Node) -> String {
        self.line(prefix, &node.label, &node.cells, node.item.is_none())
    }

    pub fn line(
        &self,
        prefix: &str,
        label: &str,
        cells: &[String; COLUMNS.len()],
        maybe_detail: bool,
    ) -> String {
        if maybe_detail && cells.iter().all(String::is_empty) {
            return format!("{prefix}{label}");
        }
        self.join(&format!("{prefix}{label}"), cells)
    }

    fn join(&self, name: &str, cells: &[String]) -> String {
        let mut line = format!("{name:<w$}", w = self.name);
        for (i, (w, c)) in self.cells.iter().zip(cells).enumerate() {
            if *w == 0 {
                continue;
            }
            // Counts are numbers: right-aligned.
            if i == COLUMNS.len() - 1 {
                line.push_str(&format!("  {c:>w$}"));
            } else {
                line.push_str(&format!("  {c:<w$}"));
            }
        }
        line.trim_end().to_string()
    }
}

/// Render as an aligned table with a header row; a blank line separates the project groups
/// from the global ones. Children are indented under their group.
pub fn render(nodes: &[Node]) -> String {
    if nodes.is_empty() {
        return String::new();
    }
    let widths = Widths::of(nodes, 0);
    let mut out = format!("{}\n", widths.header());
    for (i, node) in nodes.iter().enumerate() {
        if i > 0 && nodes[i - 1].scope != node.scope {
            out.push('\n');
        }
        render_rows(&widths, node, 0, &mut out);
    }
    out
}

fn render_rows(widths: &Widths, node: &Node, depth: usize, out: &mut String) {
    out.push_str(&widths.row(&"  ".repeat(depth), node));
    out.push('\n');
    for child in &node.children {
        render_rows(widths, child, depth + 1, out);
    }
}
