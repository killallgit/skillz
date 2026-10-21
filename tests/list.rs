//! `skillz list`: plain table, json dump, and the group picker for queries matching several groups.

mod common;

use common::*;
use serde_json::Value;
use skillz::agents;
use skillz::cli::Deps;
use skillz::model::Scope;

const NO_SUCH_ITEM: &str = "zzzz-no-such-thing";

fn list_lines(w: &World, target: Option<&str>, all: bool) -> Vec<String> {
    let (r, p) = (Recorder::default(), Scripted::yes());
    let out = run_captured(list_cmd(target, all, false), &deps(w, &r, &p));
    String::from_utf8(out)
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with("NAME"))
        .map(Into::into)
        .collect()
}

fn is_top_level(line: &str) -> bool {
    !line.starts_with(' ')
}

/// How many items a query shows after the group picker chose `pick`, as text rows or json rows.
fn items_shown_picking(w: &World, query: &str, pick: Vec<usize>, json: bool) -> usize {
    let (r, p) = (Recorder::default(), Scripted::picking(pick));
    let d = Deps {
        interactive: true,
        ..deps(w, &r, &p)
    };
    let out = run_captured(list_cmd(Some(query), false, json), &d);
    let text = String::from_utf8(out).unwrap();
    if json {
        return serde_json::from_str::<Value>(&text)
            .unwrap()
            .as_array()
            .unwrap()
            .len();
    }
    // Items are the depth-1 rows: indented by two spaces, their details by four.
    text.lines()
        .filter(|l| l.starts_with("  ") && !l.starts_with("   "))
        .count()
}

#[test]
fn list_without_all_shows_only_group_headers() {
    let w = world();

    let collapsed = list_lines(&w, None, false);

    assert!(!collapsed.is_empty());
    assert!(collapsed.iter().all(|l| is_top_level(l)));
}

#[test]
fn list_with_all_has_the_same_groups_as_without() {
    let w = world();

    let collapsed = list_lines(&w, None, false);
    let expanded = list_lines(&w, None, true);

    assert_eq!(
        collapsed.len(),
        expanded.iter().filter(|l| is_top_level(l)).count()
    );
}

#[test]
fn list_with_all_adds_rows_under_the_groups() {
    let w = world();

    let collapsed = list_lines(&w, None, false);
    let expanded = list_lines(&w, None, true);

    assert!(expanded.len() > collapsed.len());
}

#[test]
fn a_query_expands_its_matching_group() {
    let w = world();

    let lines = list_lines(&w, Some("tdd"), false);

    assert_eq!(lines.len(), 2);
    assert!(is_top_level(&lines[0]) && !is_top_level(&lines[1]));
}

#[test]
fn a_query_without_matches_lists_a_single_row() {
    let w = world();

    assert_eq!(list_lines(&w, Some(NO_SUCH_ITEM), false).len(), 1);
}

#[test]
fn a_directory_target_lists_the_project_not_a_query() {
    let w = world();

    let project_only = list_lines(&w, Some("."), false);
    let both = list_lines(&w, None, false);

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

#[test]
fn picking_every_group_of_a_query_shows_all_its_items() {
    let w = world();

    // `bd` names three hooks (Claude, Codex, Cursor), one per "<agent> hooks" group.
    assert_eq!(items_shown_picking(&w, "bd", vec![0, 1, 2], false), 3);
}

#[test]
fn picked_groups_partition_the_items_of_a_query() {
    let w = world();
    // `s` hits the hooks and the `mattpocock/skills` skill: four groups.

    let all = items_shown_picking(&w, "s", vec![0, 1, 2, 3], false);
    let first = items_shown_picking(&w, "s", vec![0, 1, 2], false);
    let second = items_shown_picking(&w, "s", vec![3], false);

    assert!(first > 0 && second > 0);
    assert_eq!(first + second, all);
}

#[test]
fn picking_no_group_shows_no_items() {
    let w = world();

    assert_eq!(items_shown_picking(&w, "s", vec![], false), 0);
}

#[test]
fn json_output_ignores_the_group_picker() {
    let w = world();

    let all = items_shown_picking(&w, "s", vec![0, 1, 2, 3], false);

    assert_eq!(items_shown_picking(&w, "s", vec![], true), all);
}
