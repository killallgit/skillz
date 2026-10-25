use std::path::Path;

use anyhow::Context;

use serde_json::{Map, Value};

use crate::harness::read_optional::read_optional;

/// The object holding the last key of `path`, and that key.
fn json_parent<'a>(
    doc: &'a mut Value,
    path: &'a [String],
) -> Option<(&'a mut Map<String, Value>, &'a str)> {
    let (last, parents) = path.split_last()?;
    let parent = parents
        .iter()
        .try_fold(doc, |v, key| v.get_mut(key.as_str()))?;
    Some((parent.as_object_mut()?, last))
}

pub(super) fn remove_json_key(doc: &mut Value, path: &[String]) {
    if let Some((obj, last)) = json_parent(doc, path) {
        obj.shift_remove(last);
    }
}

pub(super) fn remove_array_items(doc: &mut Value, path: &[String], fields: &[(String, Value)]) {
    let Some((obj, last)) = json_parent(doc, path) else {
        return;
    };
    let Some(items) = obj.get_mut(last).and_then(Value::as_array_mut) else {
        return;
    };
    let before = items.len();
    items.retain(|item| !fields_match(item, fields));
    if items.is_empty() && before > 0 {
        obj.shift_remove(last);
    }
}

/// True when every `fields` key of `item` holds exactly the paired value.
fn fields_match(item: &Value, fields: &[(String, Value)]) -> bool {
    fields.iter().all(|(k, v)| item.get(k) == Some(v))
}

/// True when the JSON `file` holds a value at `key`; a missing file holds nothing.
pub(super) fn json_has(file: &Path, key: &[String]) -> anyhow::Result<bool> {
    let Some(text) = read_optional(file)? else {
        return Ok(false);
    };
    let doc: Value =
        serde_json::from_str(&text).with_context(|| format!("parse {}", file.display()))?;
    Ok(key
        .iter()
        .try_fold(&doc, |v, k| v.get(k.as_str()))
        .is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::action::testutil::*;
    use crate::harness::action::{Edit, apply_edit};
    use serde_json::json;

    #[test]
    fn remove_json_key_nested_and_top_level() {
        let doc = json!({"enabledPlugins": {"x@y": true, "z@y": false}, "mp": {}});
        let edit = Edit::JsonRemoveKey {
            path: strings(&["enabledPlugins", "x@y"]),
        };
        assert_eq!(
            json_after(&edit, &doc),
            json!({"enabledPlugins": {"z@y": false}, "mp": {}})
        );
        let edit = Edit::JsonRemoveKey {
            path: strings(&["mp"]),
        };
        assert_eq!(
            json_after(&edit, &doc),
            json!({"enabledPlugins": {"x@y": true, "z@y": false}})
        );
    }

    #[test]
    fn remove_array_items_matching_all_fields() {
        let doc = json!({"a": {
            "xs": [{"k": "1", "n": 2}, {"k": "1", "n": 3}, {"k": "2", "n": 2}, "scalar"],
            "one": [{"k": "1"}]
        }});
        let remove = |path: &[&str], fields: &[(&str, Value)]| Edit::JsonRemoveArrayItems {
            path: strings(path),
            fields: fields
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
        };
        let both = remove(&["a", "xs"], &[("k", json!("1")), ("n", json!(2))]);
        assert_eq!(
            json_after(&both, &doc)["a"]["xs"],
            json!([{"k": "1", "n": 3}, {"k": "2", "n": 2}, "scalar"])
        );
        let k1 = remove(&["a", "xs"], &[("k", json!("1"))]);
        assert_eq!(
            json_after(&k1, &doc)["a"]["xs"],
            json!([{"k": "2", "n": 2}, "scalar"])
        );
        let emptied = json_after(&remove(&["a", "one"], &[("k", json!("1"))]), &doc);
        assert!(emptied["a"].get("one").is_none());
        assert!(emptied["a"].get("xs").is_some());
        let src = r#"{"a":{"xs":[],"s":"x"}}"#;
        for edit in [
            remove(&["a", "xs"], &[]),
            remove(&["a", "s"], &[]),
            remove(&["a", "xs", "deeper"], &[]),
            remove(&["missing", "xs"], &[]),
        ] {
            assert_eq!(
                apply_edit(Path::new(JSON), &edit, src).unwrap(),
                src,
                "{edit:?}"
            );
        }
    }
}
