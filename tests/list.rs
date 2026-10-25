//! `skillz list`: plain table, json dump, and the group picker for queries matching several groups.

mod common;

use common::*;
use serde_json::Value;
use skillz::harness::agents;
use skillz::harness::model::Scope;

const NO_SUCH_ITEM: &str = "zzzz-no-such-thing";

#[test]
fn a_directory_target_lists_the_project_not_a_query() {
    let w = world();

    let project_only = list_json(&w, Some("."));
    let both = list_json(&w, None);

    assert!(!project_only.is_empty());
    assert!(project_only.len() < both.len());
}

#[test]
fn list_json_has_one_row_per_inventory_item() {
    let w = world();

    let rows = list_json(&w, None);

    let inventory = agents::inventory(
        &agents::registry(),
        &w.env(),
        &[Scope::Global, Scope::Project],
    );
    assert_eq!(rows.len(), inventory.items.len());
}

#[test]
fn list_json_row_describes_the_item() {
    let w = world();

    let rows = list_json(&w, None);

    let tdd: Vec<&Value> = rows.iter().filter(|i| i["name"] == "tdd").collect();
    assert_eq!(tdd.len(), 1);
    assert_eq!(tdd[0]["kind"], "skill");
    assert_eq!(tdd[0]["scope"], "global");
}

#[test]
fn list_json_row_carries_its_removal_plan() {
    let w = world();

    let rows = list_json(&w, Some("tdd"));

    assert!(!rows[0]["removal"].as_array().unwrap().is_empty());
}

#[test]
fn list_json_query_keeps_only_matches() {
    let w = world();

    assert_eq!(names_of(&list_json(&w, Some("tdd"))), ["tdd"]);
}

#[test]
fn list_json_without_matches_is_empty() {
    let w = world();

    assert!(list_json(&w, Some(NO_SUCH_ITEM)).is_empty());
}
