use anyhow::Context;

pub(super) fn toml_set_key(contents: &str, path: &[String], value: &str) -> anyhow::Result<String> {
    let mut doc: toml_edit::DocumentMut = contents.parse().context("invalid TOML")?;
    let mut value: toml_edit::Value = value.parse().context("invalid TOML value")?;
    let (last, parents) = path.split_last().context("empty TOML path")?;
    let mut parent = doc.as_item_mut();
    for key in parents {
        let table = parent
            .as_table_like_mut()
            .context("TOML parent is not a table")?;
        if !table.contains_key(key) {
            let mut child = toml_edit::Table::new();
            child.set_implicit(true);
            table.insert(key, toml_edit::Item::Table(child));
        }
        parent = table.get_mut(key).context("missing TOML parent")?;
    }
    let table = parent
        .as_table_like_mut()
        .context("TOML parent is not a table")?;
    if let Some(existing) = table.get(last).and_then(toml_edit::Item::as_value) {
        *value.decor_mut() = existing.decor().clone();
        if existing.to_string() == value.to_string() {
            return Ok(contents.to_string());
        }
    }
    table.insert(last, toml_edit::Item::Value(value));
    Ok(doc.to_string())
}

pub(super) fn toml_remove_key(contents: &str, path: &[String]) -> anyhow::Result<String> {
    let mut doc: toml_edit::DocumentMut = contents.parse().context("invalid TOML")?;
    let Some((last, parents)) = path.split_last() else {
        return Ok(contents.to_string());
    };
    let parent = parents
        .iter()
        .try_fold(doc.as_item_mut(), |item, key| item.get_mut(key.as_str()));
    let removed = parent
        .and_then(toml_edit::Item::as_table_like_mut)
        .and_then(|table| table.remove(last));
    Ok(match removed {
        Some(_) => doc.to_string(),
        None => contents.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use crate::action::testutil::*;
    use crate::action::{Edit, apply_edit};
    use std::path::Path;

    #[test]
    fn setting_toml_keys_preserves_siblings_and_handles_dotted_and_inline_tables() {
        let edit = Edit::TomlSetKey {
            path: strings(&["plugins", "p@m", "enabled"]),
            value: "true".into(),
        };
        for before in [
            "model = \"o3\"\n",
            "model = \"o3\"\nplugins.\"p@m\".enabled = false\n",
            "model = \"o3\"\nplugins = { \"p@m\" = { enabled = false }, keep = { enabled = false } }\n",
        ] {
            let after = apply_edit(Path::new(TOML), &edit, before).unwrap();
            let doc: toml_edit::DocumentMut = after.parse().unwrap();
            assert_eq!(doc["model"].as_str(), Some("o3"));
            assert_eq!(doc["plugins"]["p@m"]["enabled"].as_bool(), Some(true));
            if let Some(keep) = doc["plugins"].get("keep") {
                assert_eq!(keep["enabled"].as_bool(), Some(false));
            }
            assert_eq!(apply_edit(Path::new(TOML), &edit, &after).unwrap(), after);
        }
    }

    #[test]
    fn setting_toml_keys_preserves_comments_and_rejects_invalid_structure() {
        let edit = Edit::TomlSetKey {
            path: strings(&["plugins", "p@m", "enabled"]),
            value: "true".into(),
        };
        let before = "# comment\n[plugins.\"p@m\"]\nenabled = true # keep\n";
        assert_eq!(apply_edit(Path::new(TOML), &edit, before).unwrap(), before);
        assert!(apply_edit(Path::new(TOML), &edit, "plugins = false\n").is_err());
        assert!(
            apply_edit(
                Path::new(TOML),
                &Edit::TomlSetKey {
                    path: vec![],
                    value: "true".into()
                },
                ""
            )
            .is_err()
        );
        assert!(
            apply_edit(
                Path::new(TOML),
                &Edit::TomlSetKey {
                    path: strings(&["a"]),
                    value: "invalid!".into()
                },
                ""
            )
            .is_err()
        );
    }

    #[test]
    fn toml_remove_table_keeps_formatting() {
        let edit = Edit::TomlRemoveKey {
            path: strings(&["plugins", "x@mp"]),
        };
        let out = apply_edit(Path::new(TOML), &edit, CODEX).unwrap();
        let doc: toml_edit::DocumentMut = out.parse().unwrap();
        assert!(doc["plugins"].get("x@mp").is_none());
        assert_eq!(doc["plugins"]["keep@mp"]["enabled"].as_bool(), Some(false));
        assert!(out.starts_with("# top comment\nmodel = \"o3\"\n"));
        assert!(out.contains("source_type = \"git\" # inline comment"));
    }

    #[test]
    fn toml_remove_dotted_and_inline_keys() {
        let edit = Edit::TomlRemoveKey {
            path: strings(&["plugins", "x@mp"]),
        };
        let dotted = "plugins.\"x@mp\".enabled = true\nplugins.\"k@mp\".enabled = true\n";
        let doc = toml_after(&edit, dotted);
        assert!(doc["plugins"].get("x@mp").is_none());
        assert!(doc["plugins"].get("k@mp").is_some());

        let inline = "plugins = { \"x@mp\" = { enabled = true }, other = 1 }\n";
        let doc = toml_after(&edit, inline);
        assert!(doc["plugins"].get("x@mp").is_none());
        assert_eq!(doc["plugins"]["other"].as_integer(), Some(1));

        let whole = Edit::TomlRemoveKey {
            path: strings(&["marketplaces", "mp"]),
        };
        let doc = toml_after(&whole, CODEX);
        assert!(doc.get("marketplaces").and_then(|m| m.get("mp")).is_none());
        assert!(doc["plugins"].get("x@mp").is_some());
    }

    #[test]
    fn toml_noops_return_input_verbatim() {
        for path in [
            strings(&["plugins", "absent"]),
            strings(&["model", "x"]),
            vec![],
        ] {
            assert_eq!(
                apply_edit(Path::new(TOML), &Edit::TomlRemoveKey { path }, CODEX).unwrap(),
                CODEX
            );
        }
    }
}
