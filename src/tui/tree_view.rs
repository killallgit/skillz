use std::collections::BTreeSet;

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, List, ListState, Paragraph};

use super::{FilterKey, event_loop, filter_key, is_ctrl_c, step_cursor};
use crate::matcher::fuzzy_rank;
use crate::tree::Node;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Continue,
    Quit,
    /// Picker only: ordinals of the chosen top-level nodes, ascending.
    Accept(Vec<usize>),
}

/// One node of the flattened (preorder) tree.
#[derive(Debug, Clone)]
struct Row {
    label: String,
    depth: usize,
    parent: Option<usize>,
    has_children: bool,
}

fn flatten(nodes: &[Node], depth: usize, parent: Option<usize>, rows: &mut Vec<Row>) {
    for node in nodes {
        let id = rows.len();
        rows.push(Row {
            label: node.label.clone(),
            depth,
            parent,
            has_children: !node.children.is_empty(),
        });
        flatten(&node.children, depth + 1, Some(id), rows);
    }
}

/// Collapsible tree state. `cursor` indexes `visible`, which holds row ids in preorder.
#[derive(Debug, Clone)]
pub struct TreeState {
    rows: Vec<Row>,
    expanded: BTreeSet<usize>,
    cursor: usize,
    filter: String,
    filtering: bool,
    visible: Vec<usize>,
    /// `Some` in picker mode: marked top-level row ids.
    marks: Option<BTreeSet<usize>>,
}

impl TreeState {
    pub fn new(nodes: &[Node]) -> Self {
        let mut rows = Vec::new();
        flatten(nodes, 0, None, &mut rows);
        let expanded = if nodes.len() <= 3 {
            (0..rows.len()).filter(|&i| rows[i].depth == 0).collect()
        } else {
            BTreeSet::new()
        };
        let mut state = TreeState {
            rows,
            expanded,
            cursor: 0,
            filter: String::new(),
            filtering: false,
            visible: vec![],
            marks: None,
        };
        state.refresh();
        state
    }

    /// Pick top-level nodes: starts collapsed; space expands, x/tab marks, enter accepts.
    pub fn picker(nodes: &[Node]) -> Self {
        let mut state = TreeState::new(nodes);
        state.expanded.clear();
        state.marks = Some(BTreeSet::new());
        state.refresh();
        state
    }

    /// Top-level row id that `id` sits under.
    fn top(&self, mut id: usize) -> usize {
        while let Some(p) = self.rows[id].parent {
            id = p;
        }
        id
    }

    /// Picker result: marked groups, else the one under the cursor; as top-level ordinals.
    fn accepted(&self, marks: &BTreeSet<usize>) -> Vec<usize> {
        let ids: BTreeSet<usize> = match self.current() {
            Some(id) if marks.is_empty() => [self.top(id)].into(),
            _ => marks.clone(),
        };
        let tops = (0..self.rows.len()).filter(|&i| self.rows[i].depth == 0);
        tops.enumerate()
            .filter_map(|(ordinal, id)| ids.contains(&id).then_some(ordinal))
            .collect()
    }

    pub fn handle(&mut self, key: KeyEvent) -> Outcome {
        if is_ctrl_c(key) {
            return Outcome::Quit;
        }
        if self.filtering {
            match filter_key(&mut self.filter, key.code) {
                FilterKey::Edited => {
                    self.cursor = 0;
                    self.refresh();
                }
                FilterKey::Left => {
                    self.filtering = false;
                    self.refresh();
                }
                FilterKey::Other => self.navigate(key.code),
            }
            return Outcome::Continue;
        }
        if let (Some(id), Some(marks)) = (self.current(), &self.marks) {
            match key.code {
                KeyCode::Enter => return Outcome::Accept(self.accepted(marks)),
                KeyCode::Char('x') | KeyCode::Tab => {
                    let top = self.top(id);
                    let marks = self.marks.as_mut().expect("picker mode");
                    if !marks.remove(&top) {
                        marks.insert(top);
                    }
                    return Outcome::Continue;
                }
                _ => {}
            }
        }
        match key.code {
            KeyCode::Char('/') => self.filtering = true,
            KeyCode::Esc if !self.filter.is_empty() => {
                self.filter.clear();
                self.refresh();
            }
            KeyCode::Esc | KeyCode::Char('q') => return Outcome::Quit,
            code => self.navigate(code),
        }
        Outcome::Continue
    }

    fn navigate(&mut self, code: KeyCode) {
        let Some(id) = self.current() else { return };
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.cursor = step_cursor(self.cursor, self.visible.len(), true)
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.cursor = step_cursor(self.cursor, self.visible.len(), false)
            }
            KeyCode::Right | KeyCode::Char('l') if self.rows[id].has_children => {
                self.expanded.insert(id);
            }
            KeyCode::Enter | KeyCode::Char(' ') if self.rows[id].has_children => {
                if !self.expanded.remove(&id) {
                    self.expanded.insert(id);
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if !self.expanded.remove(&id)
                    && let Some(p) = self.rows[id].parent
                {
                    self.cursor = self.visible.iter().position(|&v| v == p).unwrap_or(0);
                }
            }
            _ => return,
        }
        self.refresh();
    }

    fn current(&self) -> Option<usize> {
        self.visible.get(self.cursor).copied()
    }

    fn is_open(&self, id: usize) -> bool {
        !self.filter.is_empty() || self.expanded.contains(&id)
    }

    fn refresh(&mut self) {
        self.visible = if self.filter.is_empty() {
            let mut shown = vec![false; self.rows.len()];
            for (i, row) in self.rows.iter().enumerate() {
                shown[i] = row
                    .parent
                    .is_none_or(|p| shown[p] && self.expanded.contains(&p));
            }
            (0..self.rows.len()).filter(|&i| shown[i]).collect()
        } else {
            let labels: Vec<String> = self.rows.iter().map(|r| r.label.clone()).collect();
            let mut keep = BTreeSet::new();
            for mut id in fuzzy_rank(&self.filter, &labels) {
                while keep.insert(id) {
                    match self.rows[id].parent {
                        Some(p) => id = p,
                        None => break,
                    }
                }
            }
            keep.into_iter().collect()
        };
        self.cursor = self.cursor.min(self.visible.len().saturating_sub(1));
    }

    fn draw(&self, title: &str, frame: &mut Frame) {
        let [body, footer] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(frame.area());
        let items = self.visible.iter().map(|&id| {
            let row = &self.rows[id];
            let marker = match (row.has_children, self.is_open(id)) {
                (false, _) => " ",
                (true, true) => "▾",
                (true, false) => "▸",
            };
            let mark = match &self.marks {
                Some(m) if row.depth == 0 => ["[ ] ", "[x] "][usize::from(m.contains(&id))],
                _ => "",
            };
            format!("{}{mark}{marker} {}", "  ".repeat(row.depth), row.label)
        });
        let list = List::new(items)
            .block(Block::bordered().title(title.to_string()))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED));
        let mut list_state = ListState::default().with_selected(Some(self.cursor));
        frame.render_stateful_widget(list, body, &mut list_state);
        let help = if self.filtering {
            format!("/{}", self.filter)
        } else if self.marks.is_some() {
            "↑↓/jk move · space/→← expand · x/tab mark · enter show · / filter · esc cancel".into()
        } else {
            format!(
                "↑↓/jk move · →/l expand · ←/h collapse · enter/space toggle · / filter · q quit  {}",
                self.filter
            )
        };
        frame.render_widget(Paragraph::new(help), footer);
    }
}

/// Pick top-level `nodes` in a collapsible tree; returns their indices (empty = cancelled).
pub fn pick(title: &str, nodes: &[Node]) -> anyhow::Result<Vec<usize>> {
    event_loop(
        &mut TreeState::picker(nodes),
        |s, f| s.draw(title, f),
        |s, k| match s.handle(k) {
            Outcome::Continue => None,
            Outcome::Quit => Some(vec![]),
            Outcome::Accept(picked) => Some(picked),
        },
    )
}

/// Run the interactive collapsible tree browser until the user quits.
pub fn run(title: &str, nodes: &[Node]) -> anyhow::Result<()> {
    event_loop(
        &mut TreeState::new(nodes),
        |s, f| s.draw(title, f),
        |s, k| (s.handle(k) == Outcome::Quit).then_some(()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::{key, keys};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn n(label: &str, children: Vec<Node>) -> Node {
        Node {
            label: label.into(),
            children,
        }
    }

    /// g0 { a { a1, a2 }, b }, g1 { c }
    fn sample() -> Vec<Node> {
        vec![
            n(
                "g0",
                vec![
                    n("a", vec![n("a1", vec![]), n("a2", vec![])]),
                    n("b", vec![]),
                ],
            ),
            n("g1", vec![n("c", vec![])]),
        ]
    }

    fn labels(s: &TreeState) -> Vec<&str> {
        s.visible
            .iter()
            .map(|&i| s.rows[i].label.as_str())
            .collect()
    }

    fn feed(s: &mut TreeState, ks: impl IntoIterator<Item = KeyEvent>) {
        ks.into_iter()
            .for_each(|k| assert_eq!(s.handle(k), Outcome::Continue));
    }

    #[test]
    fn few_groups_start_expanded_many_start_collapsed() {
        let s = TreeState::new(&sample());
        assert_eq!(labels(&s), ["g0", "a", "b", "g1", "c"]);
        let many: Vec<Node> = (0..4)
            .map(|i| n(&format!("g{i}"), vec![n("x", vec![])]))
            .collect();
        assert_eq!(TreeState::new(&many).visible.len(), 4);
    }

    #[test]
    fn expand_toggle_and_collapse() {
        let mut s = TreeState::new(&sample());
        feed(&mut s, keys("jl"));
        assert_eq!(labels(&s), ["g0", "a", "a1", "a2", "b", "g1", "c"]);
        feed(&mut s, [key(KeyCode::Enter)]);
        assert_eq!(s.visible.len(), 5);
        feed(&mut s, keys(" "));
        assert_eq!(s.visible.len(), 7);
        feed(&mut s, [key(KeyCode::Left)]);
        assert_eq!(s.visible.len(), 5);
        assert_eq!(s.cursor, 1);
    }

    #[test]
    fn left_on_leaf_jumps_to_parent_then_collapses() {
        let mut s = TreeState::new(&sample());
        feed(&mut s, keys("jjjj"));
        assert_eq!(labels(&s)[s.cursor], "c");
        feed(&mut s, keys("h"));
        assert_eq!(labels(&s)[s.cursor], "g1");
        feed(&mut s, keys("h"));
        assert_eq!(labels(&s), ["g0", "a", "b", "g1"]);
        feed(&mut s, keys("h"));
        assert_eq!(s.cursor, 3);
    }

    #[test]
    fn leaf_toggles_are_noops_and_cursor_clamps() {
        let mut s = TreeState::new(&sample());
        feed(&mut s, keys("jjl "));
        assert_eq!(s.visible.len(), 5);
        feed(&mut s, keys("jjjjjj"));
        assert_eq!(s.cursor, 4);
        feed(&mut s, keys("kkkkkkkk"));
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn filter_shows_matches_with_ancestors_and_esc_clears() {
        let mut s = TreeState::new(&sample());
        feed(&mut s, keys("/a2"));
        assert_eq!(labels(&s), ["g0", "a", "a2"]);
        assert!(s.is_open(1));
        feed(&mut s, [key(KeyCode::Enter)]);
        assert!(!s.filtering);
        assert_eq!(s.visible.len(), 3);
        feed(&mut s, [key(KeyCode::Esc)]);
        assert_eq!(labels(&s), ["g0", "a", "b", "g1", "c"]);
        assert_eq!(s.handle(key(KeyCode::Esc)), Outcome::Quit);
    }

    #[test]
    fn filter_mode_esc_clears_and_letters_are_text() {
        let mut s = TreeState::new(&sample());
        feed(&mut s, keys("/qzz"));
        assert!(s.visible.is_empty());
        feed(
            &mut s,
            [
                key(KeyCode::Down),
                key(KeyCode::Backspace),
                key(KeyCode::Esc),
            ],
        );
        assert!(!s.filtering && s.filter.is_empty());
        assert_eq!(s.visible.len(), 5);
        assert_eq!(s.handle(key(KeyCode::Char('q'))), Outcome::Quit);
    }

    #[test]
    fn picker_starts_collapsed_space_expands_and_enter_takes_the_current_group() {
        let mut s = TreeState::picker(&sample());
        assert_eq!(labels(&s), ["g0", "g1"]);
        feed(&mut s, keys(" "));
        assert_eq!(labels(&s), ["g0", "a", "b", "g1"]);
        feed(&mut s, keys("jjj"));
        assert_eq!(s.handle(key(KeyCode::Enter)), Outcome::Accept(vec![1]));
    }

    #[test]
    fn picker_marks_groups_even_from_a_child_row() {
        let mut s = TreeState::picker(&sample());
        feed(&mut s, keys(" jx"));
        feed(&mut s, [key(KeyCode::Tab)]);
        feed(&mut s, keys("jjx"));
        assert_eq!(s.handle(key(KeyCode::Enter)), Outcome::Accept(vec![1]));
        feed(&mut s, keys("kkkx"));
        assert_eq!(s.handle(key(KeyCode::Enter)), Outcome::Accept(vec![0, 1]));
        assert_eq!(s.handle(key(KeyCode::Esc)), Outcome::Quit);
    }

    #[test]
    fn empty_tree_is_safe() {
        let mut s = TreeState::new(&[]);
        feed(&mut s, keys("jkhl /x"));
        assert!(s.visible.is_empty());
    }

    #[test]
    fn renders_without_panicking() {
        let mut s = TreeState::new(&sample());
        let mut term = Terminal::new(TestBackend::new(40, 8)).unwrap();
        term.draw(|f| s.draw("t", f)).unwrap();
        feed(&mut s, keys("/a"));
        term.draw(|f| s.draw("t", f)).unwrap();
    }
}
