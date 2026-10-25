use super::marks::Mark;
use super::model::Overlay;
use super::*;
use crate::tui::theme::Theme;
use crate::tui::widgets::testing::{key, keys, render_to_buffer, row_text};
use ratatui::crossterm::event::{KeyCode, KeyEvent};

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
    s.outline
        .visible()
        .iter()
        .map(|&i| s.outline.label(i))
        .collect()
}

fn under_cursor(s: &TreeState) -> &str {
    labels(s)[s.outline.selected().unwrap()]
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

fn menu_is_open(s: &TreeState) -> bool {
    matches!(s.overlay, Overlay::Menu(_))
}

#[test]
fn starts_collapsed_and_space_or_arrows_expand() {
    let mut s = TreeState::new(&sample());
    assert_eq!(labels(&s), ["g0", "g1"]);

    feed(&mut s, keys(" "));
    // Space opens the group under the cursor.
    assert_eq!(labels(&s), ["g0", "a", "b", "g1"]);

    feed(&mut s, keys("jl"));
    // Right opens a child that has children.
    assert_eq!(labels(&s), ["g0", "a", "a1", "a2", "b", "g1"]);

    feed(&mut s, [key(KeyCode::Left)]);
    // Left on an open row closes it.
    assert_eq!(s.outline.visible().len(), 4);

    feed(&mut s, keys("h"));
    // Left on a closed row climbs to its parent.
    assert_eq!(under_cursor(&s), "g0");

    feed(&mut s, keys(" "));
    assert_eq!(labels(&s), ["g0", "g1"]);
}

#[test]
fn enter_without_marks_acts_on_the_cursor_group_or_item() {
    let mut s = TreeState::new(&sample());

    let on_group = choose(&mut s, 0);

    // The first menu entry is Show, applied to the group under the cursor.
    assert_eq!(on_group, Outcome::Accept(Choice::Show, picked(&[0], &[])));
    let mut s = TreeState::new(&sample());
    feed(&mut s, keys(" jljj"));
    assert_eq!(under_cursor(&s), "a2");

    let on_detail = choose(&mut s, 1);

    // A detail leaf resolves to the item it belongs to.
    assert_eq!(
        on_detail,
        Outcome::Accept(Choice::Remove, picked(&[], &[0]))
    );
}

#[test]
fn group_mark_covers_its_items_and_unmarking_one_drops_the_group() {
    let mut s = TreeState::new(&sample());
    feed(&mut s, keys("x jx"));

    let outcome = choose(&mut s, 0);

    // g0 marked, then `a` unmarked: only `b` remains, as a single item.
    assert_eq!(outcome, Outcome::Accept(Choice::Show, picked(&[], &[1])));
    let mut s = TreeState::new(&sample());
    feed(&mut s, keys("jx"));
    feed(&mut s, [key(KeyCode::Tab)]);
    feed(&mut s, keys("x"));

    let outcome = choose(&mut s, 1);

    // Marking g1 by toggling on, off, on leaves exactly g1.
    assert_eq!(outcome, Outcome::Accept(Choice::Remove, picked(&[1], &[])));
}

#[test]
fn menu_cancel_and_esc_return_to_the_tree() {
    let mut s = TreeState::new(&sample());

    let cancelled = choose(&mut s, 2);

    // The third entry is Cancel: back to the tree.
    assert_eq!(cancelled, Outcome::Continue);
    assert!(!menu_is_open(&s));
    feed(&mut s, [key(KeyCode::Enter), key(KeyCode::Esc)]);
    assert!(!menu_is_open(&s));
    // With nothing open, Esc quits.
    assert_eq!(s.handle(key(KeyCode::Esc)), Outcome::Quit);
}

#[test]
fn left_on_leaf_jumps_to_parent_and_cursor_clamps() {
    let mut s = TreeState::new(&sample());
    feed(&mut s, keys("j jjjjjj"));
    assert_eq!(under_cursor(&s), "c");

    feed(&mut s, keys("h"));
    // A leaf's parent is the group above it.
    assert_eq!(under_cursor(&s), "g1");

    feed(&mut s, keys("kkkkkkkk"));
    // Moving up stops at the first row.
    assert_eq!(s.outline.selected(), Some(0));
}

#[test]
fn filter_shows_matches_with_ancestors_and_esc_clears() {
    let mut s = TreeState::new(&sample());

    feed(&mut s, keys("/a2"));

    // The match shows with its ancestors, and every group is open while filtering.
    assert_eq!(labels(&s), ["g0", "a", "a2"]);
    assert!(s.outline.is_open(1));
    feed(&mut s, [key(KeyCode::Enter)]);
    assert_eq!(s.overlay, Overlay::None);
    assert_eq!(s.outline.visible().len(), 3);
    feed(&mut s, [key(KeyCode::Esc)]);
    // Esc clears the filter, then q quits.
    assert_eq!(labels(&s), ["g0", "g1"]);
    assert_eq!(s.handle(key(KeyCode::Char('q'))), Outcome::Quit);
}

#[test]
fn arrows_move_the_cursor_while_the_filter_is_being_typed() {
    let mut s = TreeState::new(&sample());
    feed(&mut s, keys("/g"));

    feed(&mut s, [key(KeyCode::Down)]);

    // The filter stays active and the cursor moved to the second match.
    assert_eq!(s.overlay, Overlay::Filter);
    assert_eq!(under_cursor(&s), "g1");
}

#[test]
fn empty_tree_is_safe() {
    let mut s = TreeState::new(&[]);

    feed(&mut s, keys("jkhlx /x"));
    feed(&mut s, [key(KeyCode::Esc), key(KeyCode::Enter)]);

    // Nothing to show and no menu opened from nothing.
    assert!(s.outline.visible().is_empty() && !menu_is_open(&s));
}

#[test]
fn the_cursor_row_is_drawn_with_the_selection_style() {
    let mut s = TreeState::new(&sample());
    let theme = Theme::dark();

    let buffer = render_to_buffer(40, 8, |f| s.draw("t", f, &theme));

    // Row 0 sits under the border and the column header; its cells carry the selection background.
    assert_eq!(buffer[(6, 2)].bg, theme.selection.bg.unwrap());
    assert_ne!(buffer[(6, 3)].bg, theme.selection.bg.unwrap());
}

#[test]
fn the_menu_is_drawn_over_the_list() {
    let mut s = TreeState::new(&sample());
    let theme = Theme::dark();
    feed(&mut s, [key(KeyCode::Enter)]);

    let buffer = render_to_buffer(40, 12, |f| s.draw("t", f, &theme));

    // The modal's top-left corner is a rounded border cell, centred on the screen.
    assert_eq!(buffer[(6, 2)].symbol(), "╭");
}

#[test]
fn i_opens_the_modal_for_rows_with_info_and_esc_closes_it() {
    let mut nodes = sample();
    nodes[0].info = vec!["kind  group".into()];
    nodes[0].children[0].info = vec!["kind  skill".into()];
    let mut s = TreeState::new(&nodes);
    feed(&mut s, keys("i"));
    assert_eq!(s.overlay, Overlay::Info(0));

    feed(&mut s, keys("jx"));
    // Keys other than the closing ones do nothing while it is open.
    assert_eq!(
        (s.overlay.clone(), s.outline.selected(), s.marks.len()),
        (Overlay::Info(0), Some(0), 0)
    );
    let theme = Theme::dark();
    let buffer = render_to_buffer(60, 10, |f| s.draw("t", f, &theme));
    let shown = (0..10).any(|y| row_text(&buffer, y).contains("kind  group"));
    assert!(shown, "the info lines are drawn in the modal");

    feed(&mut s, [key(KeyCode::Esc)]);
    assert_eq!(s.overlay, Overlay::None);
    feed(&mut s, keys("ji"));
    // A row with no info (g1) has no modal.
    assert_eq!(s.overlay, Overlay::None);
}

#[test]
fn question_mark_opens_help_which_scrolls_without_moving_the_cursor_and_esc_closes_it() {
    let mut s = TreeState::new(&sample());

    feed(&mut s, keys("?"));
    let open = matches!(s.overlay, Overlay::Help(_));
    feed(&mut s, keys("j"));
    let still_open = matches!(s.overlay, Overlay::Help(_));
    feed(&mut s, [key(KeyCode::Esc)]);

    assert!(open && still_open);
    assert_eq!(
        (s.overlay.clone(), s.outline.selected()),
        (Overlay::None, Some(0))
    );
}

#[test]
fn marks_show_all_part_or_none() {
    let mut s = TreeState::new(&sample());
    assert_eq!(s.mark(0), Mark::No);

    feed(&mut s, keys(" jx"));
    // One of a group's items marked: the group is partly marked.
    assert_eq!(s.mark(0), Mark::Part);
    assert_eq!(s.mark(1), Mark::All);

    feed(&mut s, keys("kx"));
    assert_eq!(s.mark(0), Mark::All);
}

fn accepted(s: &mut TreeState, nodes: &[Node], k: KeyCode) -> Vec<usize> {
    let Outcome::Accept(_, picked) = s.handle(key(k)) else {
        panic!("enter accepts in a picker");
    };
    picked.item_indices(nodes)
}

#[test]
fn picker_enter_accepts_marks_without_the_menu() {
    let nodes = sample();
    let mut marked = TreeState::picker(&nodes);
    feed(&mut marked, keys("x"));
    let mut unmarked = TreeState::picker(&nodes);

    let with_marks = accepted(&mut marked, &nodes, KeyCode::Enter);
    let without = accepted(&mut unmarked, &nodes, KeyCode::Enter);

    // Marking g0 takes its items; with no marks, Enter takes the row under the cursor (g0 too).
    assert_eq!((with_marks, without), (vec![0, 1], vec![0, 1]));
}

#[test]
fn flat_options_pick_by_position() {
    let nodes = crate::app::tree::flat(&["a".into(), "b".into(), "c".into()]);
    let mut s = TreeState::picker(&nodes);
    feed(&mut s, keys("jx"));

    let chosen = accepted(&mut s, &nodes, KeyCode::Enter);

    // Moved to the second option and marked it.
    assert_eq!(chosen, [1]);
}

#[test]
fn filtering_underlines_the_matched_letters_and_counts_matches_in_the_badge() {
    let mut s = TreeState::new(&sample());
    feed(&mut s, keys("/c"));
    let theme = Theme::dark();

    let buffer = render_to_buffer(60, 8, |f| s.draw("t", f, &theme));

    let underlined = (0..8)
        .flat_map(|y| (0..60).map(move |x| (x, y)))
        .filter(|&pos| {
            buffer[pos]
                .modifier
                .contains(ratatui::style::Modifier::UNDERLINED)
        })
        .count();
    // Only the "c" row (and its group "g1", which has no c) is shown; exactly one letter matches.
    assert_eq!(underlined, 1);
    assert_eq!(s.outline.visible().len(), 2);
}

#[test]
fn marked_items_are_counted_through_their_group() {
    let mut s = TreeState::new(&sample());

    feed(&mut s, keys("x"));

    // Marking g0 marks its two items.
    assert_eq!(s.marked_items(), 2);
}

#[test]
fn q_and_esc_leave_a_browsing_tree_but_q_is_inert_in_the_info_modal() {
    let mut nodes = sample();
    nodes[0].info = vec!["kind  group".into()];
    let mut s = TreeState::new(&nodes);
    feed(&mut s, keys("i"));

    feed(&mut s, keys("q"));
    let still_info = s.overlay.clone();
    feed(&mut s, [key(KeyCode::Esc)]);

    assert_eq!(still_info, Overlay::Info(0));
    assert_eq!(s.handle(key(KeyCode::Char('q'))), Outcome::Quit);
}

#[test]
fn picker_enter_on_a_group_that_covers_an_item_accepts_it() {
    let nodes = vec![Node {
        covers: vec![7],
        ..n("mp (marketplace)", None, vec![n("plugin", None, vec![])])
    }];
    let mut picker = TreeState::picker(&nodes);

    assert_eq!(accepted(&mut picker, &nodes, KeyCode::Enter), [7]);
}
