//! Stores written while skillz still supported Cursor. Cursor is gone, so it is taken out of
//! every agent list; what only Cursor could use goes with it, since an empty agent list would
//! otherwise mean "the default agents".

use serde_json::{Value, json};

const REMOVED: &str = "cursor";

pub(super) fn drop_cursor(document: &mut Value) {
    if let Some(items) = document.get_mut("items").and_then(Value::as_array_mut) {
        items.retain_mut(|item| item.get_mut("entry").is_none_or(keeps_entry));
    }
    let Some(profiles) = document.get_mut("profiles").and_then(Value::as_array_mut) else {
        return;
    };
    for profile in profiles {
        if let Some(agents) = profile.get_mut("agents") {
            strip(agents);
            if agents.as_array().is_some_and(Vec::is_empty) {
                *agents = json!(["claude", "codex"]);
            }
        }
        if let Some(entries) = profile.get_mut("entry").and_then(Value::as_array_mut) {
            entries.retain_mut(keeps_entry);
        }
    }
}

/// Take Cursor out of the entry's agents; false when that leaves an entry that named only it.
fn keeps_entry(entry: &mut Value) -> bool {
    match entry.get_mut("agents") {
        Some(agents) => {
            let named = agents.as_array().is_some_and(|a| !a.is_empty());
            strip(agents);
            !named || agents.as_array().is_some_and(|a| !a.is_empty())
        }
        None => true,
    }
}

fn strip(agents: &mut Value) {
    if let Some(list) = agents.as_array_mut() {
        list.retain(|agent| agent.as_str() != Some(REMOVED));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_leaves_agent_lists_and_takes_cursor_only_things_with_it() {
        let mut document = json!({
            "version": 1,
            "items": [
                {"label": "a", "entry": {"kind": "hook", "agents": ["cursor"]}},
                {"label": "b", "entry": {"kind": "skill", "agents": ["claude", "cursor"]}},
                {"label": "c", "entry": {"kind": "skill"}}
            ],
            "profiles": [{
                "name": "web",
                "agents": ["cursor"],
                "entry": [
                    {"kind": "hook", "agents": ["cursor"]},
                    {"kind": "skill", "agents": ["codex", "cursor"]}
                ]
            }]
        });

        drop_cursor(&mut document);

        assert_eq!(
            document,
            json!({
                "version": 1,
                "items": [
                    {"label": "b", "entry": {"kind": "skill", "agents": ["claude"]}},
                    {"label": "c", "entry": {"kind": "skill"}}
                ],
                "profiles": [{
                    "name": "web",
                    "agents": ["claude", "codex"],
                    "entry": [{"kind": "skill", "agents": ["codex"]}]
                }]
            })
        );
    }
}
