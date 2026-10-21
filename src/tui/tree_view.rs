use std::collections::BTreeSet;

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Flex, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Clear, List, ListState, Paragraph};

use super::{FilterKey, Mark, draw_list, event_loop, filter_key, is_ctrl_c, step_cursor};
use crate::matcher::fuzzy_rank;
use crate::prompt::{Choice, Picked};
use crate::tree::{COLUMNS, Node, Widths};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Continue,
    Quit,
    Accept(Choice, Picked),
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
    info: Vec<String>,
    cells: [String; COLUMNS.len()],
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
    /// A picker: Enter accepts the marks directly instead of opening the action menu.
    pick: bool,
    widths: Widths,
    /// The row whose info modal is open.
    info: Option<usize>,
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
            pick: false,
            widths: Widths::of(nodes, 2),
            info: None,
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

    pub fn handle(&mut self, key: KeyEvent) -> Outcome {
        if is_ctrl_c(key) {
            return Outcome::Quit;
        }
        if self.info.is_some() {
            if matches!(
                key.code,
                KeyCode::Esc | KeyCode::Char('q' | 'i') | KeyCode::Enter
            ) {
                self.info = None;
            }
            return Outcome::Continue;
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
            KeyCode::Char('i') => {
                self.info = self.current().filter(|&id| !self.rows[id].info.is_empty())
            }
            KeyCode::Enter if self.current().is_some() || !self.marks.is_empty() => {
                if self.pick {
                    return Outcome::Accept(Choice::Remove, self.picked());
                }
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
                let prefix = format!("{}{caret} ", "  ".repeat(row.depth));
                let text = self
                    .widths
                    .line(&prefix, &row.label, &row.cells, row.item.is_none());
                format!("{}{text}", self.mark(id).column())
            })
            .collect();
        let footer = if self.filtering {
            format!("/{}", self.filter)
        } else {
            let enter = if self.pick {
                "enter confirm"
            } else {
                "enter actions"
            };
            format!(
                "↑↓/jk move · space/→← expand · x/tab mark · i info · {enter} · / filter · q quit  {}",
                self.filter
            )
        };
        let body = draw_list(
            frame,
            title,
            &format!("  {}", self.widths.header()),
            rows,
            self.cursor,
            footer,
        );

        if let Some(id) = self.info {
            let row = &self.rows[id];
            let height = (row.info.len() as u16 + 2).min(body.height);
            let [area] = Layout::vertical([Constraint::Length(height)])
                .flex(Flex::Center)
                .areas(body);
            let [area] = Layout::horizontal([Constraint::Percentage(80)])
                .flex(Flex::Center)
                .areas(area);
            let modal = Paragraph::new(row.info.join("\n"))
                .block(Block::bordered().title(format!(" {} · esc closes ", row.label)));
            frame.render_widget(Clear, area);
            frame.render_widget(modal, area);
        }

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
            Outcome::Continue => None,
            Outcome::Quit => Some(None),
            Outcome::Accept(choice, picked) => Some(Some((choice, picked))),
        },
    )
}

/// Pick items from `nodes` (marking groups or items); the chosen `Node::item` indices,
/// ascending. Empty when the user quits.
pub fn pick(title: &str, nodes: &[Node]) -> anyhow::Result<Vec<usize>> {
    event_loop(
        &mut TreeState::picker(nodes),
        |s, f| s.draw(title, f),
        |s, k| match s.handle(k) {
            Outcome::Continue => None,
            Outcome::Quit => Some(vec![]),
            Outcome::Accept(_, picked) => Some(picked.item_indices(nodes)),
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
            ..Node::default()
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
    fn i_opens_the_modal_for_rows_with_info_and_esc_closes_it() {
        let mut nodes = sample();
        nodes[0].info = vec!["kind  group".into()];
        nodes[0].children[0].info = vec!["kind  skill".into()];
        let mut s = TreeState::new(&nodes);
        feed(&mut s, keys("i"));
        assert_eq!(s.info, Some(0));
        // Keys other than the closing ones do nothing while it is open.
        feed(&mut s, keys("jx"));
        assert_eq!((s.info, s.cursor, s.marks.len()), (Some(0), 0, 0));
        let mut term = Terminal::new(TestBackend::new(60, 10)).unwrap();
        term.draw(|f| s.draw("t", f)).unwrap();
        feed(&mut s, [key(KeyCode::Esc)]);
        assert_eq!(s.info, None);
        // A row with no info (g1) has no modal.
        feed(&mut s, keys("j"));
        feed(&mut s, keys("i"));
        assert_eq!(s.info, None);
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

    #[test]
    fn picker_enter_accepts_marks_without_the_menu() {
        let nodes = sample();
        let mut s = TreeState::picker(&nodes);
        // Mark group g0 (all its items), expand nothing, accept.
        for k in keys("x") {
            assert_eq!(s.handle(k), Outcome::Continue);
        }
        let Outcome::Accept(_, picked) = s.handle(key(KeyCode::Enter)) else {
            panic!("enter accepts in a picker");
        };
        assert_eq!(picked.item_indices(&nodes), [0, 1]);
        // With no marks, Enter takes the row under the cursor.
        let mut s = TreeState::picker(&nodes);
        let Outcome::Accept(_, picked) = s.handle(key(KeyCode::Enter)) else {
            panic!("enter accepts in a picker");
        };
        assert_eq!(picked.item_indices(&nodes), [0, 1]);
    }

    #[test]
    fn flat_options_pick_by_position() {
        let nodes = crate::tree::flat(&["a".into(), "b".into(), "c".into()]);
        let mut s = TreeState::picker(&nodes);
        for k in keys("jx") {
            s.handle(k);
        }
        let Outcome::Accept(_, picked) = s.handle(key(KeyCode::Enter)) else {
            panic!("enter accepts in a picker");
        };
        assert_eq!(picked.item_indices(&nodes), [1]);
        assert_eq!(crate::tree::items(&nodes).len(), 3);
    }
}
