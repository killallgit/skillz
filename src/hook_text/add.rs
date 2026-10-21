//! Writing a hook handler into a hooks config: the inverse of `remove_hook`.

use anyhow::bail;
use serde_json::{Map, Value, json};

use super::{Format, HookEntry};
use crate::json_edit::edit_json;

/// Add `hook` to the nested JSON layout (Claude and Codex `hooks.json`/settings): under its
/// event, in the group with the same matcher, creating what is missing. A handler that is
/// already there is left alone, and the text stays verbatim.
pub fn add_hook(text: &str, format: Format, hook: &HookEntry) -> anyhow::Result<String> {
    if format == Format::Toml {
        bail!("adding a hook to a TOML config is not supported");
    }
    edit_json(text, |doc| insert(doc, hook))
}

fn insert(doc: &mut Value, hook: &HookEntry) {
    let Some(root) = doc.as_object_mut() else {
        return;
    };
    let events = object_at(root, "hooks");
    let groups = array_at(events, &hook.event);
    let matcher = hook.matcher.as_deref().unwrap_or("");
    match groups.iter_mut().find(|g| matcher_of(g) == matcher) {
        Some(group) => add_handler(group, &hook.command),
        None => groups.push(new_group(hook)),
    }
}

fn object_at<'a>(parent: &'a mut Map<String, Value>, key: &str) -> &'a mut Map<String, Value> {
    let slot = parent.entry(key).or_insert_with(|| json!({}));
    if !slot.is_object() {
        *slot = json!({});
    }
    slot.as_object_mut().expect("just made an object")
}

fn array_at<'a>(parent: &'a mut Map<String, Value>, key: &str) -> &'a mut Vec<Value> {
    let slot = parent.entry(key).or_insert_with(|| json!([]));
    if !slot.is_array() {
        *slot = json!([]);
    }
    slot.as_array_mut().expect("just made an array")
}

fn matcher_of(group: &Value) -> &str {
    group.get("matcher").and_then(Value::as_str).unwrap_or("")
}

fn add_handler(group: &mut Value, command: &str) {
    let Some(group) = group.as_object_mut() else {
        return;
    };
    let handlers = array_at(group, "hooks");
    let present = handlers
        .iter()
        .any(|h| h.get("command").and_then(Value::as_str) == Some(command));
    if !present {
        handlers.push(handler(command));
    }
}

fn new_group(hook: &HookEntry) -> Value {
    let mut group = Map::new();
    if let Some(matcher) = hook.matcher.as_deref().filter(|m| !m.is_empty()) {
        group.insert("matcher".into(), json!(matcher));
    }
    group.insert("hooks".into(), json!([handler(&hook.command)]));
    Value::Object(group)
}

fn handler(command: &str) -> Value {
    json!({"type": "command", "command": command})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hook_text::parse;

    fn hook(event: &str, matcher: Option<&str>, command: &str) -> HookEntry {
        HookEntry {
            event: event.into(),
            matcher: matcher.map(String::from),
            command: command.into(),
        }
    }

    fn added(text: &str, hooks: &[HookEntry]) -> Vec<HookEntry> {
        let mut text = text.to_string();
        for h in hooks {
            text = add_hook(&text, Format::Json, h).unwrap();
        }
        parse(&text, Format::Json).unwrap()
    }

    #[test]
    fn a_hook_is_added_to_an_empty_document() {
        let wanted = hook("PreToolUse", Some("Bash"), "rtk hook");

        let found = added("{}", std::slice::from_ref(&wanted));

        assert_eq!(found, [wanted]);
    }

    #[test]
    fn handlers_with_the_same_matcher_share_a_group() {
        let a = hook("PreToolUse", Some("Bash"), "a");
        let b = hook("PreToolUse", Some("Bash"), "b");

        let text = add_hook("{}", Format::Json, &a).unwrap();
        let text = add_hook(&text, Format::Json, &b).unwrap();
        let doc: Value = serde_json::from_str(&text).unwrap();

        assert_eq!(doc["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
        assert_eq!(parse(&text, Format::Json).unwrap(), [a, b]);
    }

    #[test]
    fn adding_the_same_hook_twice_changes_nothing() {
        let h = hook("SessionStart", None, "bd prime");
        let once = add_hook("{}", Format::Json, &h).unwrap();

        let twice = add_hook(&once, Format::Json, &h).unwrap();

        assert_eq!(twice, once);
    }

    #[test]
    fn other_settings_and_hooks_are_kept() {
        let existing = r#"{"model": "opus", "hooks": {"Stop": [{"hooks": [{"type": "command", "command": "old"}]}]}}"#;
        let wanted = hook("Stop", None, "new");

        let found = added(existing, std::slice::from_ref(&wanted));
        let doc: Value =
            serde_json::from_str(&add_hook(existing, Format::Json, &wanted).unwrap()).unwrap();

        assert_eq!(doc["model"], "opus");
        assert_eq!(found, [hook("Stop", None, "old"), wanted]);
    }

    #[test]
    fn toml_configs_are_not_supported() {
        let h = hook("Stop", None, "x");

        assert!(add_hook("", Format::Toml, &h).is_err());
    }
}
