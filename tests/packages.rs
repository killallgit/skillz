//! Installed items are grouped by the package they come from, as `npx skills list` does.

mod common;

use std::fs;

use common::*;
use serde_json::Value;

const SKILL: &str = "---\nname: NAME\ndescription: d\n---\n";
const LOCK: &str = r#"{"skills":{"tdd":{"source":"a/r","computedHash":"h"},"grill":{"source":"b/r","computedHash":"h"}}}"#;

/// A project with `tdd` from a/r and `grill` from b/r installed the way `npx skills` leaves
/// them (shared by Claude and Codex), and `mine` made by hand for Claude only.
fn sandbox_with_packages() -> Sandbox {
    let sb = Sandbox::new();
    for name in ["tdd", "grill"] {
        let store = sb.project.join(format!(".agents/skills/{name}"));
        write(&store.join("SKILL.md"), &SKILL.replace("NAME", name));
        fs::create_dir_all(sb.project.join(".claude/skills")).unwrap();
        std::os::unix::fs::symlink(&store, sb.project.join(format!(".claude/skills/{name}")))
            .unwrap();
    }
    write(&sb.project.join("skills-lock.json"), LOCK);
    write(
        &sb.project.join(".claude/skills/mine/SKILL.md"),
        &SKILL.replace("NAME", "mine"),
    );
    sb
}

fn package_of(listed: &Value, name: &str) -> Value {
    let row = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["name"] == name)
        .unwrap();
    row["package"].clone()
}

#[test]
fn json_puts_each_item_in_the_package_its_source_names() {
    let sb = sandbox_with_packages();

    let listed = sb.json(&["list", "--json"]);

    assert_eq!(package_of(&listed, "tdd"), "a/r");
    assert_eq!(package_of(&listed, "grill"), "b/r");
    assert_ne!(package_of(&listed, "mine"), package_of(&listed, "tdd"));
}

#[test]
fn uninstalling_two_packages_items_is_one_plan_that_removes_both() {
    let sb = sandbox_with_packages();

    let out = sb.skillz(&["uninstall", "tdd", "grill", "--yes"]);

    assert!(out.status.success());
    assert!(!present(sb.project.join(".agents/skills/tdd")));
    assert!(!present(sb.project.join(".agents/skills/grill")));
    assert!(present(sb.project.join(".claude/skills/mine")));
}
