//! Claude's plugin and marketplace records, parsed from its JSON files.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::files::{INSTALL_LOCATION, PLUGINS};

/// One entry of `installed_plugins.json` `plugins[id][]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Install {
    pub(super) id: String,
    /// `None` for `"scope": "user"` (global) entries, else the entry's `projectPath`.
    pub(super) project: Option<PathBuf>,
    /// The entry's `scope` as `claude plugin uninstall --scope` spells it: user, project, local.
    pub(super) scope: String,
    pub(super) install_path: Option<PathBuf>,
}

/// One entry of `known_marketplaces.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Marketplace {
    pub(super) name: String,
    pub(super) source: Option<String>,
    pub(super) install_location: String,
}

fn str_at<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

pub(super) fn installs_from(json: &Value) -> Vec<Install> {
    let Some(plugins) = json.get(PLUGINS).and_then(Value::as_object) else {
        return vec![];
    };
    plugins
        .iter()
        .flat_map(|(id, entries)| {
            let entries = entries.as_array().map(Vec::as_slice).unwrap_or_default();
            entries
                .iter()
                .filter_map(move |entry| install_from(id, entry))
        })
        .collect()
}

fn install_from(id: &str, entry: &Value) -> Option<Install> {
    let project = match str_at(entry, "scope") {
        Some("user") => None,
        _ => Some(PathBuf::from(str_at(entry, "projectPath")?)),
    };
    Some(Install {
        id: id.to_string(),
        project,
        scope: str_at(entry, "scope").unwrap_or("user").to_string(),
        install_path: str_at(entry, "installPath").map(PathBuf::from),
    })
}

pub(super) fn marketplaces_from(json: &Value) -> Vec<Marketplace> {
    let Some(markets) = json.as_object() else {
        return vec![];
    };
    markets
        .iter()
        .map(|(name, m)| Marketplace {
            name: name.clone(),
            source: marketplace_source(m),
            install_location: str_at(m, INSTALL_LOCATION).unwrap_or_default().into(),
        })
        .collect()
}

fn marketplace_source(market: &Value) -> Option<String> {
    let src = market.get("source").unwrap_or(&Value::Null);
    ["repo", "url", "path"]
        .iter()
        .find_map(|k| str_at(src, k))
        .map(String::from)
}

pub(super) fn same_path(a: &Path, b: &Path) -> bool {
    a == b || matches!((a.canonicalize(), b.canonicalize()), (Ok(x), Ok(y)) if x == y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn marketplace_source_fallbacks() {
        let ms = marketplaces_from(&json!({
            "a": {"source": {"source": "directory", "path": "/src/a"}},
            "b": {},
        }));

        assert_eq!(ms[0].source.as_deref(), Some("/src/a"));
        assert_eq!(ms[0].install_location, "");
        assert_eq!(ms[1].source, None);
        assert!(marketplaces_from(&json!([])).is_empty());
    }

    #[test]
    fn plugin_without_marketplace_suffix() {
        assert!(installs_from(&json!({"plugins": {"x@y": "nope"}})).is_empty());
        assert!(installs_from(&json!({})).is_empty());
    }
}
