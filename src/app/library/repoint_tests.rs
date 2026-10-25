//! Which stale items are re-pointed, left alone or reported.

use super::catalog::Catalog;
use super::item::LibraryItem;
use super::repoint::repoint;
use crate::app::library::Provenance;
use crate::app::stale::tests::{Existing, skill};
use skillz_harness::Spec;
use skillz_harness::model::Installer;

fn item(source: &str, name: &str) -> LibraryItem {
    LibraryItem::from_entry(skill(source, name))
}

fn live(source: &str, name: &str) -> LibraryItem {
    item(source, name).with_provenance(Provenance {
        link: Some(source.into()),
        engine: Installer::Local,
        version: None,
        recorded_at: 9,
        detected: true,
    })
}

#[test]
fn a_stale_item_is_pointed_at_the_single_live_source_in_place() {
    let mut catalog = Catalog::from_items(vec![item("/old", "mine"), item("o/r", "x")]);
    let probe = Existing::of(&["/new"]);

    let result = repoint(&mut catalog, &[live("/new", "mine")], &probe);

    let first = &catalog.items()[0];
    assert_eq!(result.moved.len(), 1);
    assert_eq!(first.entry, skill("/new", "mine"));
    assert_eq!(first.provenance.as_ref().map(|p| p.recorded_at), Some(9));
    assert_eq!(catalog.items().len(), 2);
}

#[test]
fn a_label_chosen_by_hand_survives_the_move() {
    let named = LibraryItem::new("my helper", skill("/old", "mine"));
    let mut catalog = Catalog::from_items(vec![named]);
    let probe = Existing::of(&["/new"]);

    repoint(&mut catalog, &[live("/new", "mine")], &probe);

    assert_eq!(catalog.items()[0].label, "my helper");
}

#[test]
fn two_live_sources_leave_the_item_stale_and_reported() {
    let mut catalog = Catalog::from_items(vec![item("/old", "mine")]);
    let found = [live("/a", "mine"), live("/b", "mine")];

    let result = repoint(&mut catalog, &found, &Existing::of(&["/a", "/b"]));

    assert_eq!((result.moved.len(), result.ambiguous.len()), (0, 1));
    assert_eq!(catalog.items()[0].entry, skill("/old", "mine"));
}

#[test]
fn a_stale_duplicate_of_a_live_item_is_dropped() {
    let mut catalog = Catalog::from_items(vec![item("/old", "mine"), item("/new", "mine")]);
    let probe = Existing::of(&["/new"]);

    let result = repoint(&mut catalog, &[live("/new", "mine")], &probe);

    assert_eq!(result.moved.len(), 1);
    assert_eq!(catalog.items(), [item("/new", "mine")]);
}

#[test]
fn git_sources_and_other_names_are_never_re_pointed() {
    let mut catalog = Catalog::from_items(vec![item("o/r", "mine"), item("/old", "other")]);
    let probe = Existing::of(&["/new"]);

    let result = repoint(&mut catalog, &[live("/new", "mine")], &probe);

    assert!(result.moved.is_empty() && result.ambiguous.is_empty());
}

#[test]
fn a_source_that_still_exists_is_not_stale() {
    let mut catalog = Catalog::from_items(vec![item("/here", "mine")]);
    let probe = Existing::of(&["/here", "/new"]);

    let result = repoint(&mut catalog, &[live("/new", "mine")], &probe);

    assert!(result.moved.is_empty());
}

#[test]
fn hooks_are_not_candidates() {
    let hook = LibraryItem::from_entry(Spec::Hook {
        event: "Stop".into(),
        matcher: None,
        command: "/old/run".into(),
        agents: vec![],
    });
    let mut catalog = Catalog::from_items(vec![hook]);

    let result = repoint(
        &mut catalog,
        &[live("/new", "run")],
        &Existing::of(&["/new"]),
    );

    assert!(result.moved.is_empty());
}
