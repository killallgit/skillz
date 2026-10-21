use anyhow::Context;

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
