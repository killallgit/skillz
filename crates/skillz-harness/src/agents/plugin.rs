use std::path::{Component, Path, PathBuf};

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

/// A marketplace file that is not valid JSON.
#[derive(Debug)]
struct MalformedMarketplace {
    path: PathBuf,
    source: serde_json::Error,
}

impl std::fmt::Display for MalformedMarketplace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid {}", self.path.display())
    }
}

impl std::error::Error for MalformedMarketplace {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// A marketplace file that is not valid JSON is no offer: the source's skills still install
/// instead of the whole install failing on a file the agent itself would not load. Any other
/// failure stays an error.
pub(super) fn tolerate_malformed(
    read: anyhow::Result<Option<PluginOffer>>,
) -> anyhow::Result<Option<PluginOffer>> {
    match read {
        Err(error) if error.downcast_ref::<MalformedMarketplace>().is_some() => Ok(None),
        read => read,
    }
}

/// An argument for an agent's CLI, so never option-like.
fn cli_arg(v: &serde_json::Value) -> Option<&str> {
    v.as_str().filter(|s| !s.starts_with('-'))
}

/// The marketplace at `path` reduced to what an install needs: its plugins and, for each plugin
/// whose source is local to `root`, the skill names it supplies (`skill_paths` says where a
/// plugin's skills are, which differs per agent; `manifests` are the plugin manifests to read,
/// in order). `None` when the file is missing or names no marketplace; malformed marketplace JSON
/// is an error, a malformed plugin manifest is ignored. Option-like (`-x`) names are dropped
/// since they end up as CLI args.
pub(super) fn read_marketplace(
    path: &Path,
    root: &Path,
    manifests: &[&str],
    skill_paths: fn(&serde_json::Value, Option<&serde_json::Value>, bool) -> Vec<String>,
) -> anyhow::Result<Option<PluginOffer>> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(None);
    };
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|source| MalformedMarketplace {
            path: path.to_path_buf(),
            source,
        })?;
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
    let root = root.canonicalize()?;
    for (name, plugin) in &entries {
        let source = plugin["source"].as_str().or_else(|| {
            (plugin["source"]["source"] == "local")
                .then(|| plugin["source"]["path"].as_str())
                .flatten()
        });
        let Some(source) = source.filter(|source| source.starts_with("./")) else {
            continue;
        };
        let Some(plugin_root) = contained_path(&root, source)? else {
            continue;
        };
        if !plugin_root.is_dir() {
            continue;
        }
        skills.insert(
            name.clone(),
            offered_skills(
                &plugin_root,
                manifests,
                plugin,
                plugin_root == root,
                skill_paths,
            )?,
        );
    }
    Ok(Some(PluginOffer {
        marketplace: name.into(),
        plugins: entries.into_iter().map(|(name, _)| name).collect(),
        skills,
    }))
}

fn offered_skills(
    root: &Path,
    manifests: &[&str],
    entry: &serde_json::Value,
    from_marketplace_root: bool,
    skill_paths: fn(&serde_json::Value, Option<&serde_json::Value>, bool) -> Vec<String>,
) -> anyhow::Result<Vec<String>> {
    let mut doc = None;
    for manifest in manifests {
        let Some(path) = contained_path(root, manifest)? else {
            continue;
        };
        let text = std::fs::read_to_string(path)?;
        // Ownership hints must not prevent the native installer from handling a malformed
        // manifest, especially when the user selected another plugin in the same source.
        doc = serde_json::from_str(&text).ok();
        break;
    }
    let mut names = Vec::new();
    for path in skill_paths(entry, doc.as_ref(), from_marketplace_root) {
        let Some(path) = contained_path(root, &path)? else {
            continue;
        };
        match std::fs::metadata(&path) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        }
        names.extend(crate::agent_skills::skill_names_under(&path)?);
    }
    names.sort();
    names.dedup();
    Ok(names)
}

pub(super) fn declared_skill_paths(doc: &serde_json::Value) -> Option<Vec<String>> {
    doc.get("skills").map(|paths| match paths {
        serde_json::Value::String(path) => vec![path.clone()],
        serde_json::Value::Array(paths) => paths
            .iter()
            .filter_map(|path| path.as_str().map(String::from))
            .collect(),
        _ => vec![],
    })
}

/// Reject lexical escapes and symlinks that resolve outside the source being inspected.
fn contained_path(root: &Path, relative: &str) -> anyhow::Result<Option<PathBuf>> {
    if Path::new(relative)
        .components()
        .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
    {
        return Ok(None);
    }
    let path = root.join(relative);
    let resolved = match path.canonicalize() {
        Ok(path) => path,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(None);
        }
        Err(error) => return Err(error).with_context(|| format!("reading {}", path.display())),
    };
    Ok(resolved.starts_with(root).then_some(resolved))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::{Harness, claude::Claude, codex::Codex};
    use crate::model::{Agent, Scope};
    use crate::test_support::skill;
    use serde_json::{Value, json};

    #[test]
    fn only_a_malformed_marketplace_is_tolerated() {
        let malformed = || {
            let source = serde_json::from_str::<Value>("{").unwrap_err();
            anyhow::Error::new(MalformedMarketplace {
                path: "m.json".into(),
                source,
            })
        };

        assert!(tolerate_malformed(Err(malformed())).unwrap().is_none());
        assert!(tolerate_malformed(Err(anyhow::anyhow!("permission denied"))).is_err());
    }

    fn write_json(root: &Path, path: &str, value: Value) {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, value.to_string()).unwrap();
    }

    fn marketplace(root: &Path, plugins: Value) {
        write_json(
            root,
            ".claude-plugin/marketplace.json",
            json!({"name":"mp", "plugins":plugins}),
        );
    }

    fn offer(root: &Path, agent: Agent) -> PluginOffer {
        match agent {
            Agent::Claude => Claude.plugin_offer(root, Scope::Project),
            Agent::Codex => Codex.plugin_offer(root, Scope::Project),
        }
        .unwrap()
        .unwrap()
    }

    #[test]
    fn manifest_skill_paths_extend_claudes_defaults_and_replace_codex_defaults() {
        for paths in [json!("./extras"), json!(["./extras"])] {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path();
            skill(root, "skills/alpha");
            skill(root, "extras/beta");
            marketplace(root, json!([{"name":"p", "source":"./"}]));
            write_json(
                root,
                ".claude-plugin/plugin.json",
                json!({"name":"p", "skills":paths}),
            );

            assert_eq!(offer(root, Agent::Claude).skills["p"], ["alpha", "beta"]);
            assert_eq!(offer(root, Agent::Codex).skills["p"], ["beta"]);
        }
    }

    #[test]
    fn root_sourced_marketplace_entries_expose_only_their_declared_skills() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        skill(root, "skills/alpha");
        skill(root, "skills/beta");
        marketplace(
            root,
            json!([
                {"name":"p1", "source":"./", "skills":["./skills/alpha"], "strict":false},
                {"name":"p2", "source":"./", "skills":["./skills/beta"], "strict":false}
            ]),
        );
        for agent in [Agent::Claude, Agent::Codex] {
            let offer = offer(root, agent);
            assert_eq!(offer.skills["p1"], ["alpha"]);
            assert_eq!(offer.skills["p2"], ["beta"]);
        }
    }

    #[test]
    fn entry_skill_paths_follow_each_agents_manifest_precedence() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        skill(root, "plugins/p/skills/alpha");
        skill(root, "plugins/p/extras/beta");
        skill(root, "plugins/p/entry/gamma");
        write_json(
            root,
            "plugins/p/.claude-plugin/plugin.json",
            json!({"name":"p", "skills":["./extras"]}),
        );
        for strict in [true, false] {
            marketplace(
                root,
                json!([{"name":"p", "source":"./plugins/p", "skills":"./entry", "strict":strict}]),
            );
            let claude = offer(root, Agent::Claude);
            if strict {
                assert_eq!(claude.skills["p"], ["alpha", "beta", "gamma"]);
            } else {
                assert!(claude.skills["p"].is_empty());
            }
            assert_eq!(offer(root, Agent::Codex).skills["p"], ["beta"]);
        }
    }

    #[test]
    fn component_paths_cannot_attribute_skills_outside_the_plugin() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        skill(root, "plugins/p/skills/safe");
        skill(root, "loose/outside");
        marketplace(root, json!([{"name":"p", "source":"./plugins/p"}]));
        for path in [
            "../../loose".to_string(),
            root.join("loose").to_string_lossy().into_owned(),
        ] {
            write_json(
                root,
                "plugins/p/.claude-plugin/plugin.json",
                json!({"name":"p", "skills":path}),
            );
            assert_eq!(offer(root, Agent::Claude).skills["p"], ["safe"]);
            assert!(offer(root, Agent::Codex).skills["p"].is_empty());
        }
    }

    #[test]
    fn source_paths_cannot_escape_the_marketplace() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("market");
        skill(dir.path(), "outside/skills/secret");
        marketplace(
            &root,
            json!([
                {"name":"parent", "source":"./../outside"},
                {"name":"absolute", "source":dir.path().join("outside").to_string_lossy()}
            ]),
        );
        for agent in [Agent::Claude, Agent::Codex] {
            assert!(offer(&root, agent).skills.is_empty());
        }
    }

    #[cfg(unix)]
    #[test]
    fn source_and_component_symlinks_cannot_escape_their_roots() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("market");
        skill(dir.path(), "outside/skills/secret");
        skill(&root, "plugins/p/skills/safe");
        symlink(dir.path().join("outside"), root.join("escape")).unwrap();
        symlink(
            dir.path().join("outside/skills"),
            root.join("plugins/p/extra"),
        )
        .unwrap();
        marketplace(
            &root,
            json!([{"name":"escape", "source":"./escape"}, {"name":"p", "source":"./plugins/p"}]),
        );
        write_json(
            &root,
            "plugins/p/.claude-plugin/plugin.json",
            json!({"name":"p", "skills":"./extra"}),
        );
        let claude = offer(&root, Agent::Claude);
        let codex = offer(&root, Agent::Codex);
        assert!(!claude.skills.contains_key("escape"));
        assert!(!codex.skills.contains_key("escape"));
        assert_eq!(claude.skills["p"], ["safe"]);
        assert!(codex.skills["p"].is_empty());
    }

    #[test]
    fn malformed_manifests_do_not_block_other_plugin_offers() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        skill(root, "plugins/bad/skills/alpha");
        skill(root, "plugins/good/skills/beta");
        marketplace(
            root,
            json!([{"name":"bad", "source":"./plugins/bad"}, {"name":"good", "source":"./plugins/good"}]),
        );
        std::fs::create_dir_all(root.join("plugins/bad/.claude-plugin")).unwrap();
        std::fs::write(root.join("plugins/bad/.claude-plugin/plugin.json"), "{").unwrap();
        for agent in [Agent::Claude, Agent::Codex] {
            let offer = offer(root, agent);
            assert_eq!(offer.skills["bad"], ["alpha"]);
            assert_eq!(offer.skills["good"], ["beta"]);
        }
    }
}
