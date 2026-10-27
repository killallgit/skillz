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
pub(super) fn read_marketplace(
    path: &Path,
    root: &Path,
    manifests: &[&str],
) -> anyhow::Result<Option<PluginOffer>> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(None);
    };
    let json: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("invalid {}", path.display()))?;
    let Some(name) = cli_arg(&json["name"]) else {
        return Ok(None);
    };
    let entries: Vec<_> = json["plugins"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| cli_arg(&p["name"]).map(|name| (name.to_string(), p)))
        .collect();
    let mut skills = std::collections::BTreeMap::new();
    for (name, plugin) in &entries {
        let source = plugin["source"].as_str().or_else(|| {
            (plugin["source"]["source"] == "local")
                .then(|| plugin["source"]["path"].as_str())
                .flatten()
        });
        let Some(source) = source.filter(|source| source.starts_with("./")) else {
            continue;
        };
        skills.insert(name.clone(), offered_skills(&root.join(source), manifests)?);
    }
    Ok(Some(PluginOffer {
        marketplace: name.into(),
        plugins: entries.into_iter().map(|(name, _)| name).collect(),
        skills,
    }))
}

fn offered_skills(root: &Path, manifests: &[&str]) -> anyhow::Result<Vec<String>> {
    let mut paths = vec!["./skills".to_string()];
    for manifest in manifests {
        let Some(text) = crate::read_optional::read_optional(&root.join(manifest))? else {
            continue;
        };
        let doc: serde_json::Value = serde_json::from_str(&text)?;
        if let Some(skills) = doc.get("skills") {
            paths = match skills {
                serde_json::Value::String(path) => vec![path.clone()],
                serde_json::Value::Array(paths) => paths
                    .iter()
                    .filter_map(|path| path.as_str().map(String::from))
                    .collect(),
                _ => vec![],
            };
        }
        break;
    }
    let mut names = Vec::new();
    for path in paths {
        let path = root.join(path);
        match std::fs::metadata(&path) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        }
        crate::fswalk::walk(&path, &mut |path, is_dir| {
            if is_dir {
                names.extend(crate::agent_skills::skill_name(path));
            }
        })?;
    }
    names.sort();
    names.dedup();
    Ok(names)
}
