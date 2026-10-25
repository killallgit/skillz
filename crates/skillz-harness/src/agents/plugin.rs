use std::path::Path;

use anyhow::Context;

use super::PluginOffer;
use crate::read_optional::read_dir;

pub(super) fn skill_names(root: &Path) -> anyhow::Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in read_dir(&root.join("skills"))? {
        match std::fs::metadata(entry.path().join("SKILL.md")) {
            Ok(metadata) if metadata.is_file() => {
                if let Some(name) = entry.file_name().to_str() {
                    names.push(name.to_string());
                }
            }
            Ok(_) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) => {}
            Err(error) => return Err(error.into()),
        }
    }
    names.sort();
    Ok(names)
}

/// An argument for an agent's CLI, so never option-like.
fn cli_arg(v: &serde_json::Value) -> Option<&str> {
    v.as_str().filter(|s| !s.starts_with('-'))
}

/// The marketplace at `path` reduced to what an install needs. `None` when the file is missing
/// or names no marketplace; malformed JSON is an error. Option-like (`-x`) names are dropped
/// since they end up as CLI args.
pub(super) fn read_marketplace(path: &Path) -> anyhow::Result<Option<PluginOffer>> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(None);
    };
    let json: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("invalid {}", path.display()))?;
    let Some(name) = cli_arg(&json["name"]) else {
        return Ok(None);
    };
    let plugins = json["plugins"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| cli_arg(&p["name"]).map(String::from))
        .collect();
    Ok(Some(PluginOffer {
        marketplace: name.into(),
        plugins,
    }))
}
