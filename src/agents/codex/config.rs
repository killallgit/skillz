use std::path::Path;

use anyhow::Context;
use toml_edit::{DocumentMut, Item};

use crate::env::Env;

pub(super) fn read_config(path: &Path) -> anyhow::Result<Option<DocumentMut>> {
    match std::fs::read_to_string(path) {
        Ok(text) => text
            .parse::<DocumentMut>()
            .map(Some)
            .with_context(|| format!("parsing {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

pub(super) fn table_keys<'a>(doc: &'a DocumentMut, key: &str) -> Vec<(&'a str, &'a Item)> {
    doc.get(key)
        .and_then(Item::as_table_like)
        .map(|t| t.iter().collect())
        .unwrap_or_default()
}

pub(super) fn str_field<'a>(item: &'a Item, key: &str) -> Option<&'a str> {
    item.get(key).and_then(Item::as_str)
}

/// Plugin ids the current project's config lists (enabled or not), so a global removal keeps
/// the shared cache they load from; `None` (keep every cache) when it cannot be read. Empty
/// when that config is `config` itself (project scope, or run from home).
// shortcut: other projects' configs are not visible, so their use is not protected; upgrade
// if Codex ever records which projects use a cached plugin.
pub(super) fn project_plugin_ids(env: &Env, config: &Path) -> Option<Vec<String>> {
    let project = env.project.join(".codex/config.toml");
    if std::fs::canonicalize(&project).ok() == std::fs::canonicalize(config).ok() {
        return Some(vec![]);
    }
    let doc = read_config(&project).ok()?;
    Some(doc.as_ref().map(plugin_ids).unwrap_or_default())
}

// Ids without `@mp` are not a shape Codex installs; skip rather than guess a cache path.
pub(super) fn plugin_ids(doc: &DocumentMut) -> Vec<String> {
    table_keys(doc, "plugins")
        .into_iter()
        .map(|(id, _)| id.to_string())
        .filter(|id| id.contains('@'))
        .collect()
}

/// `[hooks.state."<id>:..."]` trust entries Codex keeps for a plugin's hooks.
pub(super) fn hook_state_keys(doc: &DocumentMut, id: &str) -> Vec<String> {
    let prefix = format!("{id}:");
    doc.get("hooks")
        .and_then(|h| h.get("state"))
        .and_then(Item::as_table_like)
        .map(|t| {
            t.iter()
                .map(|(k, _)| k)
                .filter(|k| k.starts_with(&prefix))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// `name@mp` -> (`name`, `mp`).
pub(super) fn split_id(id: &str) -> (&str, &str) {
    id.rsplit_once('@').unwrap_or((id, ""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_state_keys_match_plugin_prefix_only() {
        let key = "ponytail@ponytail:hooks/claude-codex-hooks.json:session_start:0:0";
        let doc: DocumentMut = format!(
            "[hooks.state.\"{key}\"]\ntrusted_hash = \"h\"\nenabled = true\n\
             [hooks.state.\"ponytail@other:hooks/x.json:stop:0:0\"]\nenabled = true\n"
        )
        .parse()
        .unwrap();
        assert_eq!(hook_state_keys(&doc, "ponytail@ponytail"), vec![key]);
        assert!(hook_state_keys(&doc, "ponytail").is_empty());
        assert!(hook_state_keys(&"x = 1".parse().unwrap(), "a@b").is_empty());
    }
}
