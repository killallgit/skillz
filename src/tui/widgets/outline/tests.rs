use ratatui::crossterm::event::KeyCode;

use super::*;
use crate::tui::keymap::Action;
use crate::tui::widgets::input::Outcome as Edit;
use crate::tui::widgets::testing::open_all;
use crate::tui::widgets::testing::{key, keys};

/// 0 fruit { 1 apple, 2 apricot }, 3 veg { 4 leek }
fn shape() -> Vec<Shape> {
    [
        (None, "fruit"),
        (Some(0), "apple"),
        (Some(0), "apricot"),
        (None, "veg"),
        (Some(3), "leek"),
    ]
    .map(|(p, l)| (p, l.to_string()))
    .to_vec()
}

fn expanded() -> Outline {
    let mut outline = Outline::new(shape());
    open_all(&mut outline);
    outline
}

fn type_query(outline: &mut Outline, query: &str) {
    keys(query).into_iter().for_each(|k| {
        outline.filter_key(k);
    });
}

fn visible_labels(outline: &Outline) -> Vec<&str> {
    outline
        .visible()
        .iter()
        .map(|&id| outline.label(id))
        .collect()
}

#[test]
fn a_new_outline_shows_only_its_top_rows() {
    let outline = Outline::new(shape());

    assert_eq!(outline.visible(), [0, 3]);
}

#[test]
fn opening_every_row_shows_them_in_preorder() {
    assert_eq!(expanded().visible(), [0, 1, 2, 3, 4]);
}

#[test]
fn right_opens_a_closed_row_and_left_closes_it_or_climbs_to_the_parent() {
    let mut outline = Outline::new(shape());

    outline.navigate(Action::Right);
    let opened = outline.visible().len();
    outline.navigate(Action::Down);
    outline.navigate(Action::Left);
    let climbed = outline.current();
    outline.navigate(Action::Left);

    assert_eq!((opened, climbed), (4, Some(0)));
    assert_eq!(outline.visible(), [0, 3]);
}

#[test]
fn toggling_a_leaf_does_nothing() {
    let mut outline = expanded();
    outline.select_row(1);

    outline.navigate(Action::Toggle);

    assert_eq!(outline.visible().len(), 5);
}

#[test]
fn depth_counts_the_ancestors() {
    let outline = expanded();

    assert_eq!((outline.depth(0), outline.depth(1)), (0, 1));
}

#[test]
fn a_filter_keeps_matches_with_their_ancestors_even_when_closed() {
    let mut outline = Outline::new(shape());

    type_query(&mut outline, "leek");

    assert_eq!(outline.visible(), [3, 4]);
    assert!(outline.is_open(3));
}

#[test]
fn submitting_keeps_the_filter_and_cancelling_restores_every_row() {
    let mut outline = expanded();
    type_query(&mut outline, "apr");

    let submitted = outline.filter_key(key(KeyCode::Enter));
    let kept = outline.visible().to_vec();
    let cancelled = outline.filter_key(key(KeyCode::Esc));

    assert_eq!((submitted, cancelled), (Edit::Submit, Edit::Cancel));
    assert_eq!(kept, [0, 2]);
    assert_eq!(outline.visible().len(), 5);
}

#[test]
fn reshaping_keeps_open_rows_open_by_label_and_starts_new_ones_closed() {
    let mut outline = Outline::new(shape());
    outline.navigate(Action::Right);
    let mut grown = vec![(None, "bread".to_string()), (Some(0), "rye".to_string())];
    grown.extend(shape().into_iter().map(|(p, l)| (p.map(|p| p + 2), l)));

    outline.reshape(grown);

    assert_eq!(
        visible_labels(&outline),
        ["bread", "fruit", "apple", "apricot", "veg"]
    );
}

#[test]
fn reopening_a_parent_reveals_only_its_immediate_children() {
    let mut outline = Outline::new(vec![
        (None, "root".into()),
        (Some(0), "group".into()),
        (Some(1), "nested".into()),
        (Some(2), "item".into()),
    ]);
    open_all(&mut outline);
    outline.toggle_open(0);
    outline.toggle_open(0);
    assert_eq!(outline.visible(), [0, 1]);
    outline.select_row(1);
    outline.navigate(Action::Right);
    assert_eq!(outline.visible(), [0, 1, 2]);
    outline.select_row(2);
    outline.navigate(Action::Right);
    assert_eq!(outline.visible(), [0, 1, 2, 3]);
}

#[test]
fn reshaping_keeps_same_named_groups_in_other_branches_closed() {
    let tree = vec![
        (None, "first".into()),
        (Some(0), "shared".into()),
        (Some(1), "a".into()),
        (None, "second".into()),
        (Some(3), "shared".into()),
        (Some(4), "b".into()),
    ];
    let mut outline = Outline::new(tree.clone());
    outline.toggle_open(0);
    outline.toggle_open(1);
    outline.toggle_open(3);
    outline.reshape(tree);
    assert_eq!(outline.visible(), [0, 1, 2, 3, 4]);
    assert!(!outline.is_open(4));
}

#[test]
fn the_badge_mentions_the_total_only_while_filtered() {
    let mut outline = expanded();
    let plain = outline.badge();

    type_query(&mut outline, "ap");

    assert!(!plain.contains("of"));
    assert!(outline.badge().contains("of"));
}

#[test]
fn toggling_while_filtered_leaves_what_was_open_for_when_the_filter_clears() {
    let mut outline = Outline::new(shape());
    type_query(&mut outline, "apple");

    outline.toggle_open(0);
    outline.clear_filter();

    assert_eq!(outline.visible(), [0, 3]);
}
