use anyhow::Context;
use serde_json::Value;

use super::{Format, HookEntry};
use crate::json_edit::edit_json;

/// Parse the hooks of a config file. JSON: the top-level `"hooks"` object, nested
/// (Claude/Codex) or flat layout. TOML: the inline hooks of a Codex `config.toml`.
/// A document without hooks has none.
pub fn parse(text: &str, format: Format) -> anyhow::Result<Vec<HookEntry>> {
    let mut out = Vec::new();
    let collect = |e| {
        out.push(e);
        true
    };
    match format {
        Format::Json => {
            let mut doc: Value = serde_json::from_str(text).context("invalid hooks JSON")?;
            walk_json(&mut doc, collect);
        }
        Format::Toml => {
            let mut doc: toml_edit::DocumentMut = text.parse().context("invalid hooks TOML")?;
            walk_toml(&mut doc, collect);
        }
    }
    Ok(out)
}

/// Remove every handler under `event` whose command equals `command`, pruning emptied groups
/// and events. JSON keeps key order; TOML keeps formatting. Returns `text` verbatim when
/// nothing matches.
pub fn remove_hook(
    text: &str,
    format: Format,
    event: &str,
    command: &str,
) -> anyhow::Result<String> {
    let keep = |e: HookEntry| e.event != event || e.command != command;
    match format {
        Format::Json => edit_json(text, |doc| walk_json(doc, keep)),
        Format::Toml => {
            let mut doc: toml_edit::DocumentMut = text.parse().context("invalid TOML")?;
            Ok(if walk_toml(&mut doc, keep) {
                doc.to_string()
            } else {
                text.to_string()
            })
        }
    }
}

/// Visit every handler with a command in `hooks.<Event>[]`: a nested group's `hooks[]`, or
/// the flat entry itself. Drops handlers `keep` rejects, then the groups and events that
/// emptied.
fn walk_json(doc: &mut Value, mut keep: impl FnMut(HookEntry) -> bool) {
    let Some(events) = doc.get_mut("hooks").and_then(Value::as_object_mut) else {
        return;
    };
    events.retain(|event, groups| {
        let Some(groups) = groups.as_array_mut() else {
            return true;
        };
        let before = groups.len();
        groups.retain_mut(|group| {
            let matcher = matcher(group.get("matcher").and_then(Value::as_str));
            let mut keep_handler = |h: &Value| match h.get("command").and_then(Value::as_str) {
                Some(command) => keep(entry(event, &matcher, command)),
                None => true,
            };
            match group.get_mut("hooks").and_then(Value::as_array_mut) {
                Some(handlers) => {
                    let before = handlers.len();
                    handlers.retain(|h| keep_handler(h));
                    !handlers.is_empty() || before == 0
                }
                None => keep_handler(group),
            }
        });
        !groups.is_empty() || before == 0
    });
}

/// Visit every handler in `[[hooks.<Event>]]` + `[[hooks.<Event>.hooks]]` (or the inline-array
/// equivalent). Non-array values under `hooks` (e.g. the `hooks.state` trust table) are not
/// events. Drops handlers `keep` rejects, then the groups and events that emptied; returns
/// whether anything was dropped.
fn walk_toml(doc: &mut toml_edit::DocumentMut, mut keep: impl FnMut(HookEntry) -> bool) -> bool {
    let Some(events) = doc
        .get_mut("hooks")
        .and_then(toml_edit::Item::as_table_like_mut)
    else {
        return false;
    };
    let mut changed = false;
    let mut emptied = Vec::new();
    for (event, groups) in events.iter_mut() {
        let event = event.get().to_string();
        let (before, after) = retain_tables(groups, |group| {
            let matcher = matcher(group.get("matcher").and_then(toml_edit::Item::as_str));
            let Some(handlers) = group.get_mut("hooks") else {
                return true;
            };
            let (b, a) = retain_tables(handlers, |h| {
                match h.get("command").and_then(toml_edit::Item::as_str) {
                    Some(command) => keep(entry(&event, &matcher, command)),
                    None => true,
                }
            });
            changed |= a != b;
            a > 0 || b == 0
        });
        if after == 0 && before > 0 {
            emptied.push(event);
        }
    }
    for event in emptied {
        events.remove(&event);
    }
    changed
}

/// Keep the tables of `[[x]]` or `x = [{..}, ..]` that `keep` approves (it may also edit
/// them); other values stay. Returns the table count before and after.
fn retain_tables(
    item: &mut toml_edit::Item,
    mut keep: impl FnMut(&mut dyn toml_edit::TableLike) -> bool,
) -> (usize, usize) {
    let keeps: Vec<bool> = match item {
        toml_edit::Item::ArrayOfTables(a) => a.iter_mut().map(|t| keep(t)).collect(),
        toml_edit::Item::Value(toml_edit::Value::Array(a)) => a
            .iter_mut()
            .filter_map(toml_edit::Value::as_inline_table_mut)
            .map(|t| keep(t))
            .collect(),
        _ => return (0, 0),
    };
    let mut next = keeps.iter().copied();
    match item {
        toml_edit::Item::ArrayOfTables(a) => a.retain(|_| next.next().unwrap_or(true)),
        toml_edit::Item::Value(toml_edit::Value::Array(a)) => {
            a.retain(|v| !v.is_inline_table() || next.next().unwrap_or(true))
        }
        _ => {}
    }
    (keeps.len(), keeps.iter().filter(|k| **k).count())
}

/// An empty matcher is the same as none.
fn matcher(m: Option<&str>) -> Option<String> {
    m.filter(|m| !m.is_empty()).map(str::to_string)
}

fn entry(event: &str, matcher: &Option<String>, command: &str) -> HookEntry {
    HookEntry {
        event: event.to_string(),
        matcher: matcher.clone(),
        command: command.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::*;
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_nested_layout() {
        let doc = json!({"hooks": {
            "PreToolUse": [{"matcher": "Bash", "hooks": [
                {"type": "command", "command": "rtk hook claude"},
                {"type": "prompt", "prompt": "no command"}
            ]}],
            "SessionStart": [{"matcher": "", "hooks": [{"type": "command", "command": "bd prime"}]}]
        }});
        assert_eq!(
            parse(&doc.to_string(), Format::Json).unwrap(),
            vec![
                hook_entry("PreToolUse", Some("Bash"), "rtk hook claude"),
                hook_entry("SessionStart", None, "bd prime"),
            ]
        );
    }

    #[test]
    fn parses_flat_layout() {
        let doc = json!({"version": 1, "hooks": {"sessionStart": [
            {"command": "bd cursor-hook sessionStart"},
            {"command": "x", "matcher": "Shell"}
        ], "bogus": {"not": "an array"}}});
        assert_eq!(
            parse(&doc.to_string(), Format::Json).unwrap(),
            vec![
                hook_entry("sessionStart", None, "bd cursor-hook sessionStart"),
                hook_entry("sessionStart", Some("Shell"), "x"),
            ]
        );
    }

    #[test]
    fn no_hooks_key_or_invalid_json() {
        assert!(
            parse(r#"{"enabledPlugins": {}}"#, Format::Json)
                .unwrap()
                .is_empty()
        );
        assert!(parse(r#"{"hooks": []}"#, Format::Json).unwrap().is_empty());
        assert!(parse("{nope", Format::Json).is_err());
    }

    #[test]
    fn parses_toml_layouts_and_skips_state() {
        assert_eq!(
            parse(CODEX_TOML, Format::Toml).unwrap(),
            vec![
                hook_entry(
                    "SessionStart",
                    Some("startup|resume"),
                    "bd codex-hook SessionStart"
                ),
                hook_entry("Stop", None, "rtk hook codex"),
                hook_entry("Stop", None, "bd codex-hook Stop"),
            ]
        );
        assert!(
            parse("[features]\nx = 1\n", Format::Toml)
                .unwrap()
                .is_empty()
        );
        assert!(parse("hooks = 1\n", Format::Toml).unwrap().is_empty());
        assert!(parse("[nope", Format::Toml).is_err());
    }

    /// Removes every parsed entry in turn.
    fn remove_all(text: &str, format: Format) -> String {
        parse(text, format)
            .unwrap()
            .iter()
            .fold(text.to_string(), |t, e| {
                remove_hook(&t, format, &e.event, &e.command).unwrap()
            })
    }

    #[test]
    fn toml_round_trip_removes_hooks_only() {
        let text = remove_all(CODEX_TOML, Format::Toml);
        assert!(parse(&text, Format::Toml).unwrap().is_empty());
        assert!(text.starts_with("# my config\n"));
        let doc: toml_edit::DocumentMut = text.parse().unwrap();
        assert_eq!(doc["features"]["codex_hooks"].as_bool(), Some(true));
        assert_eq!(doc["plugins"]["x@y"]["enabled"].as_bool(), Some(true));
        assert_eq!(
            doc["hooks"]["state"]["x@y:hooks.json:0"]["trusted"].as_bool(),
            Some(true)
        );
        let hooks: Vec<_> = doc["hooks"]
            .as_table_like()
            .unwrap()
            .iter()
            .map(|(k, _)| k)
            .collect();
        assert_eq!(hooks, ["state"]);
    }

    #[test]
    fn json_round_trip_removes_hooks_only() {
        let doc = json!({"z": 1, "hooks": {
            "PreToolUse": [{"matcher": "Bash", "hooks": [
                {"command": "rtk"}, {"type": "prompt", "prompt": "stays"}
            ]}],
            "stop": [{"command": "bd"}, {"command": "x", "matcher": "Shell"}],
            "Stop": [{"hooks": []}]
        }, "m": 2});
        let after: Value =
            serde_json::from_str(&remove_all(&doc.to_string(), Format::Json)).unwrap();
        assert_eq!(
            after,
            json!({"z": 1, "hooks": {
                "PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "prompt", "prompt": "stays"}]}],
                "Stop": [{"hooks": []}]
            }, "m": 2})
        );
    }

    fn json_removed(doc: &Value, event: &str, command: &str) -> Value {
        let out = remove_hook(&doc.to_string(), Format::Json, event, command).unwrap();
        serde_json::from_str(&out).unwrap()
    }

    #[test]
    fn remove_hook_nested_prunes_groups_and_events() {
        let doc = json!({"other": 1, "hooks": {
            "PreToolUse": [
                {"matcher": "Bash", "hooks": [{"command": "rtk"}, {"command": "keep"}]},
                {"matcher": "Edit", "hooks": [{"command": "rtk"}]}
            ],
            "SessionStart": [{"hooks": [{"command": "rtk"}]}],
            "Stop": [{"hooks": []}]
        }});
        let after = json_removed(&doc, "PreToolUse", "rtk");
        assert_eq!(
            after["hooks"]["PreToolUse"],
            json!([{"matcher": "Bash", "hooks": [{"command": "keep"}]}])
        );
        let after = json_removed(&after, "SessionStart", "rtk");
        assert_eq!(
            after,
            json!({"other": 1, "hooks": {
                "PreToolUse": [{"matcher": "Bash", "hooks": [{"command": "keep"}]}],
                "Stop": [{"hooks": []}]
            }})
        );
    }

    #[test]
    fn remove_hook_flat_and_keeps_empty_hooks_object() {
        let doc = json!({"version": 1, "hooks": {"stop": [{"command": "bd"}, {"command": "bd"}]}});
        assert_eq!(
            json_removed(&doc, "stop", "bd"),
            json!({"version": 1, "hooks": {}})
        );
    }

    #[test]
    fn remove_hook_preserves_key_order() {
        let src =
            r#"{"z": 1, "hooks": {"b": [{"command": "x"}], "a": [{"command": "y"}]}, "m": 2}"#;
        let out: Value =
            serde_json::from_str(&remove_hook(src, Format::Json, "b", "x").unwrap()).unwrap();
        let keys: Vec<_> = out.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, ["z", "hooks", "m"]);
        assert_eq!(out["hooks"], json!({"a": [{"command": "y"}]}));
    }

    #[test]
    fn json_remove_hook_absent_is_verbatim() {
        let src = "{\"hooks\":{\"a\":[{\"command\":\"x\"}]},\"plugins\":{}}";
        for src in [src, "{}", r#"{"hooks": 3}"#] {
            for (event, command) in [("missing", "x"), ("a", "other")] {
                assert_eq!(remove_hook(src, Format::Json, event, command).unwrap(), src);
            }
        }
        assert!(remove_hook("{oops", Format::Json, "a", "x").is_err());
    }

    fn toml_removed(src: &str, event: &str, command: &str) -> toml_edit::DocumentMut {
        remove_hook(src, Format::Toml, event, command)
            .unwrap()
            .parse()
            .unwrap()
    }

    #[test]
    fn toml_remove_hook_prunes_groups_and_events() {
        let out = remove_hook(HOOKS_TOML, Format::Toml, "SessionStart", "rtk").unwrap();
        assert!(out.contains("# start hooks\n") && out.contains("# keep me"));
        let doc: toml_edit::DocumentMut = out.parse().unwrap();
        let groups = doc["hooks"]["SessionStart"].as_array_of_tables().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups.get(0).unwrap()["matcher"].as_str(), Some("startup"));
        let handlers = groups.get(0).unwrap()["hooks"]
            .as_array_of_tables()
            .unwrap();
        let commands: Vec<_> = handlers.iter().map(|h| h["command"].as_str()).collect();
        assert_eq!(commands, [Some("keep")]);

        let doc = toml_removed(&out, "SessionStart", "keep");
        assert!(doc["hooks"].get("SessionStart").is_none());
        assert_eq!(doc["features"]["x"].as_integer(), Some(1));
        assert_eq!(doc["plugins"]["a@b"]["enabled"].as_bool(), Some(true));
        assert_eq!(
            doc["hooks"]["state"]["a@b:x"]["trusted"].as_bool(),
            Some(true)
        );
    }

    #[test]
    fn toml_remove_hook_inline_keeps_empty_and_odd_entries() {
        let doc = toml_removed(HOOKS_TOML, "Stop", "rtk");
        let stop = doc["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 2);
        assert_eq!(stop.get(1).and_then(|v| v.as_str()), Some("odd"));
    }

    #[test]
    fn toml_remove_hook_absent_is_verbatim() {
        for (event, command) in [("Nope", "rtk"), ("SessionStart", "nope"), ("state", "rtk")] {
            assert_eq!(
                remove_hook(HOOKS_TOML, Format::Toml, event, command).unwrap(),
                HOOKS_TOML
            );
        }
        assert_eq!(
            remove_hook("a = 1\n", Format::Toml, "E", "c").unwrap(),
            "a = 1\n"
        );
        assert!(remove_hook("[nope", Format::Toml, "E", "c").is_err());
    }
}
