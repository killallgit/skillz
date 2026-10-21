use std::collections::BTreeSet;

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Flex, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Clear, List, ListState};

use super::{FilterKey, Mark, draw_list, event_loop, filter_key, is_ctrl_c, step_cursor};
use crate::matcher::fuzzy_rank;
use crate::prompt::{Choice, Picked};
use crate::tree::Node;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Continue,
    Quit,
    Accept(Choice, Picked),
    /// `i` on this row (an index into the tree's rows).
    Info(usize),
}

/// Menu entries shown on Enter; `None` closes the menu.
const MENU: [(&str, Option<Choice>); 3] = [
    ("Show", Some(Choice::Show)),
    ("Remove…", Some(Choice::Remove)),
    ("Cancel", None),
];

/// One node of the flattened (preorder) tree.
#[derive(Debug, Clone)]
struct Row {
    label: String,
    depth: usize,
    parent: Option<usize>,
    has_children: bool,
    item: Option<usize>,
}

fn flatten(nodes: &[Node], depth: usize, parent: Option<usize>, rows: &mut Vec<Row>) {
    for node in nodes {
        let id = rows.len();
        rows.push(Row {
            label: node.label.clone(),
            depth,
            parent,
            has_children: !node.children.is_empty(),
            item: node.item,
        });
        flatten(&node.children, depth + 1, Some(id), rows);
    }
}

/// Collapsible, markable tree. `cursor` indexes `visible`, which holds row ids in preorder.
/// Groups (top-level rows) and item rows can be marked; Enter opens the action menu.
#[derive(Debug, Clone)]
pub struct TreeState {
    rows: Vec<Row>,
    expanded: BTreeSet<usize>,
    marks: BTreeSet<usize>,
    cursor: usize,
    filter: String,
    filtering: bool,
    visible: Vec<usize>,
    /// Cursor into `MENU` while the action menu is open.
    menu: Option<usize>,
}

impl TreeState {
    pub fn new(nodes: &[Node]) -> Self {
        let mut rows = Vec::new();
        flatten(nodes, 0, None, &mut rows);
        let mut state = TreeState {
            rows,
            expanded: BTreeSet::new(),
            marks: BTreeSet::new(),
            cursor: 0,
            filter: String::new(),
            filtering: false,
            visible: vec![],
            menu: None,
        };
        state.refresh();
        state
    }

    pub fn handle(&mut self, key: KeyEvent) -> Outcome {
        if is_ctrl_c(key) {
            return Outcome::Quit;
        }
        if let Some(at) = self.menu {
            return self.handle_menu(at, key.code);
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
        match key.code {
            KeyCode::Char('/') => self.filtering = true,
            KeyCode::Char('x') | KeyCode::Tab => self.toggle_mark(),
            KeyCode::Char('i') => return self.current().map_or(Outcome::Continue, Outcome::Info),
            KeyCode::Enter if self.current().is_some() || !self.marks.is_empty() => {
                self.menu = Some(0)
            }
            KeyCode::Esc if !self.filter.is_empty() => {
                self.filter.clear();
                self.refresh();
            }
            KeyCode::Esc | KeyCode::Char('q') => return Outcome::Quit,
            code => self.navigate(code),
        }
        Outcome::Continue
    }

    fn handle_menu(&mut self, at: usize, code: KeyCode) -> Outcome {
        match code {
            KeyCode::Up | KeyCode::Char('k') => self.menu = Some(step_cursor(at, MENU.len(), true)),
            KeyCode::Down | KeyCode::Char('j') => {
                self.menu = Some(step_cursor(at, MENU.len(), false))
            }
            KeyCode::Enter => match MENU[at].1 {
                Some(choice) => return Outcome::Accept(choice, self.picked()),
                None => self.menu = None,
            },
            KeyCode::Esc | KeyCode::Char('q') => self.menu = None,
            _ => {}
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
            KeyCode::Char(' ') if self.rows[id].has_children => {
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

    /// The markable row `id` belongs to: itself, its item, or its group.
    fn target(&self, mut id: usize) -> usize {
        while self.rows[id].depth > 0 && self.rows[id].item.is_none() {
            id = self.rows[id].parent.expect("non-top rows have a parent");
        }
        id
    }

    fn group_of(&self, mut id: usize) -> usize {
        while let Some(p) = self.rows[id].parent {
            id = p;
        }
        id
    }

    fn items_in(&self, group: usize) -> Vec<usize> {
        (0..self.rows.len())
            .filter(|&i| self.rows[i].item.is_some() && self.group_of(i) == group)
            .collect()
    }

    /// x on a group marks or unmarks all its items; on an item, just that item.
    fn toggle_mark(&mut self) {
        let Some(id) = self.current() else { return };
        let id = self.target(id);
        let on = !self.marks.contains(&id);
        if self.rows[id].depth == 0 {
            for i in self.items_in(id).into_iter().chain([id]) {
                if on {
                    self.marks.insert(i);
                } else {
                    self.marks.remove(&i);
                }
            }
        } else if on {
            self.marks.insert(id);
        } else {
            self.marks.remove(&id);
            self.marks.remove(&self.group_of(id));
        }
    }

    /// Marked groups (as top-level ordinals) and items outside them; with no marks, whatever
    /// the cursor is on.
    fn picked(&self) -> Picked {
        let marks: BTreeSet<usize> = match self.current() {
            Some(id) if self.marks.is_empty() => [self.target(id)].into(),
            _ => self.marks.clone(),
        };
        let tops: Vec<usize> = (0..self.rows.len())
            .filter(|&i| self.rows[i].depth == 0)
            .collect();
        Picked {
            groups: (0..tops.len())
                .filter(|&o| marks.contains(&tops[o]))
                .collect(),
            items: marks
                .iter()
                .filter(|&&i| !marks.contains(&self.group_of(i)))
                .filter_map(|&i| self.rows[i].item)
                .collect(),
        }
    }

    fn mark(&self, id: usize) -> Mark {
        let row = &self.rows[id];
        if self.marks.contains(&id) {
            Mark::All
        } else if row.depth == 0 && self.items_in(id).iter().any(|i| self.marks.contains(i)) {
            Mark::Part
        } else {
            Mark::No
        }
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
        let rows = self
            .visible
            .iter()
            .map(|&id| {
                let row = &self.rows[id];
                let caret = match (row.has_children, self.is_open(id)) {
                    (false, _) => " ",
                    (true, true) => "▾",
                    (true, false) => "▸",
                };
                format!(
                    "{}{}{caret} {}",
                    self.mark(id).column(),
                    "  ".repeat(row.depth),
                    row.label
                )
            })
            .collect();
        let footer = if self.filtering {
            format!("/{}", self.filter)
        } else {
            format!(
                "↑↓/jk move · space/→← expand · x/tab mark · i info · enter actions · / filter · q quit  {}",
                self.filter
            )
        };
        let body = draw_list(frame, title, rows, self.cursor, footer);

        if let Some(at) = self.menu {
            let [area] = Layout::vertical([Constraint::Length(MENU.len() as u16 + 2)])
                .flex(Flex::Center)
                .areas(body);
            let [area] = Layout::horizontal([Constraint::Length(24)])
                .flex(Flex::Center)
                .areas(area);
            let menu = List::new(MENU.iter().map(|(label, _)| *label))
                .block(Block::bordered().title("Do what?"))
                .highlight_style(Style::new().add_modifier(Modifier::REVERSED));
            frame.render_widget(Clear, area);
            frame.render_stateful_widget(
                menu,
                area,
                &mut ListState::default().with_selected(Some(at)),
            );
        }
    }
}

/// Browse `nodes`, mark groups or items, and pick an action; `None` when the user quits.
pub fn browse(title: &str, nodes: &[Node]) -> anyhow::Result<Option<(Choice, Picked)>> {
    event_loop(
        &mut TreeState::new(nodes),
        |s, f| s.draw(title, f),
        |s, k| match s.handle(k) {
            // The info modal (skillz-atj) takes over `Info`.
            Outcome::Continue | Outcome::Info(_) => None,
            Outcome::Quit => Some(None),
            Outcome::Accept(choice, picked) => Some(Some((choice, picked))),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::{key, keys};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn n(label: &str, item: Option<usize>, children: Vec<Node>) -> Node {
        Node {
            label: label.into(),
            children,
            item,
            scope: None,
        }
    }

    /// g0 { a#0 { a1, a2 }, b#1 }, g1 { c#2 }
    fn sample() -> Vec<Node> {
        vec![
            n(
                "g0",
                None,
                vec![
                    n(
                        "a",
                        Some(0),
                        vec![n("a1", None, vec![]), n("a2", None, vec![])],
                    ),
                    n("b", Some(1), vec![]),
                ],
            ),
            n("g1", None, vec![n("c", Some(2), vec![])]),
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

    /// Enter, then the `n`th menu entry.
    fn choose(s: &mut TreeState, n: usize) -> Outcome {
        feed(s, [key(KeyCode::Enter)]);
        feed(s, (0..n).map(|_| key(KeyCode::Down)));
        s.handle(key(KeyCode::Enter))
    }

    fn picked(groups: &[usize], items: &[usize]) -> Picked {
        Picked {
            groups: groups.to_vec(),
            items: items.to_vec(),
        }
    }

    #[test]
    fn starts_collapsed_and_space_or_arrows_expand() {
        let mut s = TreeState::new(&sample());
        assert_eq!(labels(&s), ["g0", "g1"]);
        feed(&mut s, keys(" "));
        assert_eq!(labels(&s), ["g0", "a", "b", "g1"]);
        feed(&mut s, keys("jl"));
        assert_eq!(labels(&s), ["g0", "a", "a1", "a2", "b", "g1"]);
        feed(&mut s, [key(KeyCode::Left)]);
        assert_eq!(s.visible.len(), 4);
        feed(&mut s, keys("h"));
        assert_eq!(labels(&s)[s.cursor], "g0");
        feed(&mut s, keys(" "));
        assert_eq!(labels(&s), ["g0", "g1"]);
    }

    #[test]
    fn enter_without_marks_acts_on_the_cursor_group_or_item() {
        let mut s = TreeState::new(&sample());
        assert_eq!(
            choose(&mut s, 0),
            Outcome::Accept(Choice::Show, picked(&[0], &[]))
        );
        let mut s = TreeState::new(&sample());
        feed(&mut s, keys(" jljj"));
        assert_eq!(labels(&s)[s.cursor], "a2");
        assert_eq!(
            choose(&mut s, 1),
            Outcome::Accept(Choice::Remove, picked(&[], &[0]))
        );
    }

    #[test]
    fn group_mark_covers_its_items_and_unmarking_one_drops_the_group() {
        let mut s = TreeState::new(&sample());
        feed(&mut s, keys("x jx"));
        // g0 marked, then `a` unmarked: only `b` remains, as a single item.
        assert_eq!(
            choose(&mut s, 0),
            Outcome::Accept(Choice::Show, picked(&[], &[1]))
        );
        let mut s = TreeState::new(&sample());
        feed(&mut s, keys("jx"));
        feed(&mut s, [key(KeyCode::Tab)]);
        feed(&mut s, keys("x"));
        assert_eq!(
            choose(&mut s, 1),
            Outcome::Accept(Choice::Remove, picked(&[1], &[]))
        );
    }

    #[test]
    fn menu_cancel_and_esc_return_to_the_tree() {
        let mut s = TreeState::new(&sample());
        assert_eq!(choose(&mut s, 2), Outcome::Continue);
        assert!(s.menu.is_none());
        feed(&mut s, [key(KeyCode::Enter), key(KeyCode::Esc)]);
        assert!(s.menu.is_none());
        assert_eq!(s.handle(key(KeyCode::Esc)), Outcome::Quit);
    }

    #[test]
    fn left_on_leaf_jumps_to_parent_and_cursor_clamps() {
        let mut s = TreeState::new(&sample());
        feed(&mut s, keys("j jjjjjj"));
        assert_eq!(labels(&s)[s.cursor], "c");
        feed(&mut s, keys("h"));
        assert_eq!(labels(&s)[s.cursor], "g1");
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
        assert_eq!(labels(&s), ["g0", "g1"]);
        assert_eq!(s.handle(key(KeyCode::Char('q'))), Outcome::Quit);
    }

    #[test]
    fn empty_tree_is_safe() {
        let mut s = TreeState::new(&[]);
        feed(&mut s, keys("jkhlx /x"));
        feed(&mut s, [key(KeyCode::Esc), key(KeyCode::Enter)]);
        assert!(s.visible.is_empty() && s.menu.is_none());
    }

    #[test]
    fn renders_tree_and_menu_without_panicking() {
        let mut s = TreeState::new(&sample());
        let mut term = Terminal::new(TestBackend::new(40, 8)).unwrap();
        feed(&mut s, keys(" x"));
        term.draw(|f| s.draw("t", f)).unwrap();
        feed(&mut s, [key(KeyCode::Enter)]);
        term.draw(|f| s.draw("t", f)).unwrap();
    }

    #[test]
    fn i_reports_the_row_under_the_cursor_for_every_row_kind() {
        let mut s = TreeState::new(&sample());
        assert_eq!(s.handle(key(KeyCode::Char('i'))), Outcome::Info(0));
        feed(&mut s, keys(" j"));
        assert_eq!(s.handle(key(KeyCode::Char('i'))), Outcome::Info(1));
        feed(&mut s, keys("l"));
        feed(&mut s, keys("j"));
        // a child row that is not itself markable still answers
        assert!(matches!(
            s.handle(key(KeyCode::Char('i'))),
            Outcome::Info(_)
        ));
    }

    #[test]
    fn marks_show_all_part_or_none() {
        let mut s = TreeState::new(&sample());
        assert_eq!(s.mark(0), Mark::No);
        feed(&mut s, keys(" jx"));
        assert_eq!(s.mark(0), Mark::Part);
        assert_eq!(s.mark(1), Mark::All);
        feed(&mut s, keys("kx"));
        assert_eq!(s.mark(0), Mark::All);
    }
}
