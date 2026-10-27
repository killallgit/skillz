use std::path::Path;

use toml_edit::DocumentMut;

use super::config::{hook_state_keys, plugin_ids, str_field, table_keys};
use super::removal::{native, toml_remove};
use crate::Spec;
use crate::action::{Action, Edit, RemovalTarget};
use crate::agents::PluginOffer;
use crate::agents::plugin::{declared_skill_paths, read_marketplace};
use crate::env::Env;
use crate::hooks;
use crate::model::{
    Agent, HOOK_CHILD, Installed, Installer, Kind, Origin, SKILL_CHILD, Scope, split_plugin_id,
};

/// Global installs use the CLI; project installs declare the marketplace and plugin in TOML.
pub(super) fn provision(entry: &Spec, scope: Scope, env: &Env) -> Option<Vec<Action>> {
    let Spec::Plugin {
        source,
        marketplace,
        name,
        ..
    } = entry
    else {
        return None;
    };
    if scope == Scope::Project {
        return Some(project_install_steps(source, marketplace, name, env));
    }
    Some(install_steps(
        source,
        marketplace,
        std::slice::from_ref(name),
        env,
    ))
}

/// Download Git marketplace metadata without registering or enabling anything globally.
/// Codex materializes configured plugins in its shared cache when the project is opened.
pub(super) fn project_install_steps(
    source: &str,
    marketplace: &str,
    name: &str,
    env: &Env,
) -> Vec<Action> {
    let local =
        Path::new(source).is_absolute() || source.starts_with("./") || source.starts_with("../");
    let (source, git_ref) = if local {
        (
            env.project.join(source).to_string_lossy().into_owned(),
            None,
        )
    } else if source.contains("://") || source.starts_with("git@") {
        (source.to_string(), None)
    } else {
        let (repo, git_ref) = source
            .split_once('@')
            .map_or((source, None), |(repo, git_ref)| (repo, Some(git_ref)));
        (format!("https://github.com/{repo}.git"), git_ref)
    };
    let encoded = |s: &str| toml_edit::Value::from(s).to_string();
    let mut fields = vec![
        ("source_type", encoded(if local { "local" } else { "git" })),
        ("source", encoded(&source)),
    ];
    if !local {
        // A project source must not inherit a different revision or sparse checkout from
        // a user-level marketplace with the same name.
        fields.push(("ref", encoded(git_ref.unwrap_or("HEAD"))));
        fields.push(("sparse_paths", "[]".into()));
    }
    let mut steps = Vec::new();
    if !local {
        let mut args = Vec::new();
        for (key, value) in &fields {
            args.extend([
                "-c".into(),
                format!(
                    "marketplaces.{}.{key}={value}",
                    toml_edit::Key::new(marketplace)
                ),
            ]);
        }
        args.extend(["plugin", "marketplace", "upgrade", marketplace].map(String::from));
        steps.push(Action::required("codex", args, &super::cwd(env)));
    }
    let file = super::scope_dir(env, Scope::Project).join("config.toml");
    steps.extend(fields.into_iter().map(|(key, value)| Action::Edit {
        file: file.clone(),
        edit: Edit::TomlSetKey {
            path: ["marketplaces", marketplace, key].map(String::from).into(),
            value,
        },
    }));
    steps.push(Action::Edit {
        file,
        edit: Edit::TomlSetKey {
            path: vec![
                "plugins".into(),
                format!("{name}@{marketplace}"),
                "enabled".into(),
            ],
            value: "true".into(),
        },
    });
    steps
}

/// `codex plugin marketplace add` then `plugin add` for each of `names`.
pub(super) fn install_steps(
    source: &str,
    marketplace: &str,
    names: &[String],
    env: &Env,
) -> Vec<Action> {
    let cwd = super::cwd(env);
    let run = |args: &[&str]| {
        Action::required("codex", args.iter().map(|a| a.to_string()).collect(), &cwd)
    };
    std::iter::once(run(&["plugin", "marketplace", "add", source]))
        .chain(
            names
                .iter()
                .map(|name| run(&["plugin", "add", &format!("{name}@{marketplace}")])),
        )
        .collect()
}

/// The Codex marketplace `src` carries, if any.
pub(super) fn plugin_offer(src: &Path) -> anyhow::Result<Option<PluginOffer>> {
    let manifests = &[
        "plugin.json",
        ".codex-plugin/plugin.json",
        ".claude-plugin/plugin.json",
    ];
    match read_marketplace(
        &src.join(".agents/plugins/marketplace.json"),
        src,
        manifests,
        plugin_skill_paths,
    )? {
        Some(offer) => Ok(Some(offer)),
        None => read_marketplace(
            &src.join(".claude-plugin/marketplace.json"),
            src,
            manifests,
            plugin_skill_paths,
        ),
    }
}

fn plugin_skill_paths(
    entry: &serde_json::Value,
    manifest: Option<&serde_json::Value>,
    _from_marketplace_root: bool,
) -> Vec<String> {
    // Codex uses the entry as a manifest only when the package has none; explicit skill
    // paths replace its default scan rather than extending it as Claude's paths do.
    declared_skill_paths(manifest.unwrap_or(entry)).unwrap_or_else(|| vec!["./skills".into()])
}

pub(super) struct Marketplace {
    pub name: String,
    pub source: Option<String>,
    pub bundled: bool,
}

/// What building one config's plugins and marketplaces needs to know.
pub(super) struct Cx<'a> {
    pub scope: Scope,
    pub config: &'a Path,
    pub cache: &'a Path,
    pub marketplaces: &'a [Marketplace],
    pub doc: &'a DocumentMut,
}

/// Plugins of the config, minus those of Codex's built-in marketplaces. `in_project` is the
/// project's plugin ids (`None`: unknown, keep every cache).
pub(super) fn plugins_from(
    cx: &Cx,
    in_project: Option<&[String]>,
) -> anyhow::Result<Vec<Installed>> {
    plugin_ids(cx.doc)
        .into_iter()
        .filter(|id| !is_in_bundled_marketplace(cx, id))
        .map(|id| plugin(cx, &id, project_uses(in_project, |other| other == id)))
        .collect()
}

pub(super) fn marketplaces_from(
    cx: &Cx,
    plugins: &[Installed],
    in_project: Option<&[String]>,
) -> Vec<Installed> {
    cx.marketplaces
        .iter()
        .filter(|m| !m.bundled)
        .map(|m| {
            let keep_cache = project_uses(in_project, |other| split_plugin_id(other).1 == m.name);
            marketplace(cx, m, plugins, keep_cache)
        })
        .collect()
}

fn is_in_bundled_marketplace(cx: &Cx, id: &str) -> bool {
    cx.marketplaces
        .iter()
        .any(|m| m.bundled && m.name == split_plugin_id(id).1)
}

fn project_uses(in_project: Option<&[String]>, pick: impl Fn(&str) -> bool) -> bool {
    in_project.is_none_or(|ids| ids.iter().any(|id| pick(id)))
}

pub(super) fn marketplaces(doc: &DocumentMut) -> Vec<Marketplace> {
    table_keys(doc, "marketplaces")
        .into_iter()
        .map(|(name, item)| Marketplace {
            name: name.to_string(),
            source: str_field(item, "source").map(display_source),
            bundled: is_bundled(name, str_field(item, "source_type")),
        })
        .collect()
}

// Codex ships its own `openai-*` local marketplaces; they are built in, not user-installed.
fn is_bundled(name: &str, source_type: Option<&str>) -> bool {
    source_type == Some("local") && name.starts_with("openai-")
}

fn display_source(source: &str) -> String {
    match source.strip_prefix("https://github.com/") {
        Some(repo) => repo.strip_suffix(".git").unwrap_or(repo).to_string(),
        None => source.to_string(),
    }
}

fn plugin(cx: &Cx, id: &str, keep_cache: bool) -> anyhow::Result<Installed> {
    let (name, mp) = split_plugin_id(id);
    let source = cx
        .marketplaces
        .iter()
        .find(|m| m.name == mp)
        .and_then(|m| m.source.clone());
    let cache = cx.cache.join(mp).join(name);
    let version = cached_version(&cache)?;
    let root = version.as_ref().map(|v| cache.join(v));
    let skills = root
        .as_deref()
        .map(super::super::plugin::skill_names)
        .transpose()?
        .unwrap_or_default();
    let hooks = root
        .as_deref()
        .map(|root| hooks::plugin_hooks(root, &root.join(".codex-plugin/plugin.json")))
        .unwrap_or_default();
    let mut children = skills
        .iter()
        .map(|name| format!("{SKILL_CHILD}{name}"))
        .collect::<Vec<_>>();
    children.extend(hooks.iter().map(|h| format!("{HOOK_CHILD}{}", h.label())));
    Ok(Installed {
        details: crate::model::Details::Plugin {
            location: cx.config.to_path_buf(),
            skills,
        },
        hooks,
        members: Vec::new(),
        id: format!("plugin:codex:{}:{id}", cx.scope),
        name: id.to_string(),
        kind: Kind::Plugin,
        agents: vec![Agent::Codex],
        scope: cx.scope,
        group: mp.to_string(),
        source: source.clone(),
        installer: Installer::CodexCli,
        origin: Origin {
            link: source,
            version,
        },
        children,
        removal: plugin_removal(cx, id, keep_cache),
    })
}

/// Cache revisions can be opaque names. Use the most recently modified directory;
/// the name breaks equal-time ties so every part of the plugin uses the same revision.
fn cached_version(plugin_cache: &Path) -> anyhow::Result<Option<String>> {
    let mut versions = Vec::new();
    for entry in crate::read_optional::read_dir(plugin_cache)? {
        if entry.file_type()?.is_dir()
            && let Ok(version) = entry.file_name().into_string()
        {
            versions.push((entry.metadata()?.modified()?, version));
        }
    }
    versions.sort();
    Ok(versions.pop().map(|(_, version)| version))
}

fn plugin_removal(cx: &Cx, id: &str, keep_cache: bool) -> Vec<Action> {
    let (name, mp) = split_plugin_id(id);
    let mut removal = native(cx.scope, cx.config, &["plugin", "remove", id], id);
    removal.push(toml_remove(cx.config, &["plugins", id]));
    removal.extend(
        hook_state_keys(cx.doc, id)
            .iter()
            .map(|k| toml_remove(cx.config, &["hooks", "state", k])),
    );
    if cx.scope == Scope::Global && !keep_cache {
        removal.extend(RemovalTarget::new(cx.cache, [mp, name]).map(Action::RemovePath));
    }
    removal
}

fn marketplace(cx: &Cx, m: &Marketplace, plugins: &[Installed], keep_cache: bool) -> Installed {
    let own: Vec<&Installed> = plugins.iter().filter(|p| p.group == m.name).collect();
    Installed {
        details: crate::model::Details::Marketplace {
            location: cx.config.to_path_buf(),
        },
        hooks: Vec::new(),
        members: Vec::new(),
        id: format!("marketplace:codex:{}:{}", cx.scope, m.name),
        name: m.name.clone(),
        kind: Kind::Marketplace,
        agents: vec![Agent::Codex],
        scope: cx.scope,
        group: m.name.clone(),
        source: m.source.clone(),
        installer: Installer::CodexCli,
        origin: Default::default(),
        children: own.iter().map(|p| p.name.clone()).collect(),
        removal: marketplace_removal(cx, m, &own, keep_cache),
    }
}

fn marketplace_removal(
    cx: &Cx,
    m: &Marketplace,
    own: &[&Installed],
    keep_cache: bool,
) -> Vec<Action> {
    let mut removal: Vec<Action> = own.iter().flat_map(|p| p.removal.clone()).collect();
    removal.extend(native(
        cx.scope,
        cx.config,
        &["plugin", "marketplace", "remove", &m.name],
        &m.name,
    ));
    removal.push(toml_remove(cx.config, &["marketplaces", &m.name]));
    if cx.scope == Scope::Global && !keep_cache {
        removal.extend(RemovalTarget::new(cx.cache, [&m.name]).map(Action::RemovePath));
    }
    removal
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::codex::config::read_config;
    use crate::agents::codex::testutil::write;
    use crate::agents::{Harness, codex::Codex};
    use crate::runner::NoCommands;
    use crate::test_support::FakeRunner;

    #[test]
    fn project_local_plugin_install_and_removal_preserve_other_configuration() {
        let dir = tempfile::tempdir().unwrap();
        let env = Env::new(dir.path().join("home"), dir.path().join("project"));
        let config = env.project.join(".codex/config.toml");
        let source = dir.path().join("local marketplace");
        std::fs::create_dir_all(&source).unwrap();
        write(
            &config,
            "model = \"o3\"\n[plugins.\"other@mp\"]\nenabled = false\n",
        );
        let steps = project_install_steps(&source.to_string_lossy(), "local.mp", "my-plugin", &env);

        crate::action::execute(&steps, &NoCommands).unwrap();

        let doc = read_config(&config).unwrap().unwrap();
        assert_eq!(doc["model"].as_str(), Some("o3"));
        assert_eq!(
            doc["marketplaces"]["local.mp"]["source_type"].as_str(),
            Some("local")
        );
        assert_eq!(
            doc["marketplaces"]["local.mp"]["source"].as_str(),
            source.to_str()
        );
        assert_eq!(doc["plugins"]["other@mp"]["enabled"].as_bool(), Some(false));
        let items = Codex.installed(&env, Scope::Project).unwrap();
        let plugin = items
            .iter()
            .find(|p| p.name == "my-plugin@local.mp")
            .unwrap();
        crate::action::execute(&plugin.removal, &NoCommands).unwrap();
        let doc = read_config(&config).unwrap().unwrap();
        assert!(doc["plugins"].get("my-plugin@local.mp").is_none());
        assert!(doc["plugins"].get("other@mp").is_some());
        assert!(!env.home.join(".codex/config.toml").exists());
    }

    #[test]
    fn project_git_sources_keep_urls_and_expand_shorthand_refs() {
        let dir = tempfile::tempdir().unwrap();
        let env = Env::new(dir.path().join("home"), dir.path().join("project"));
        for (source, expected, git_ref) in [
            (
                "mattpocock/skills@v1.3.1",
                "https://github.com/mattpocock/skills.git",
                Some("v1.3.1"),
            ),
            (
                "https://example.org/team/skills.git",
                "https://example.org/team/skills.git",
                None,
            ),
            (
                "git@example.org:team/skills.git",
                "git@example.org:team/skills.git",
                None,
            ),
        ] {
            let steps = project_install_steps(source, "test.mp", "skills", &env);
            let runner = FakeRunner::new(|_| {});
            crate::action::execute(&steps, &runner).unwrap();
            let doc = read_config(&env.project.join(".codex/config.toml"))
                .unwrap()
                .unwrap();
            assert_eq!(
                doc["marketplaces"]["test.mp"]["source"].as_str(),
                Some(expected)
            );
            assert_eq!(
                doc["marketplaces"]["test.mp"]["ref"].as_str(),
                Some(git_ref.unwrap_or("HEAD"))
            );
            assert!(
                doc["marketplaces"]["test.mp"]["sparse_paths"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            let calls = runner.calls();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].0, "codex");
            assert_eq!(calls[0].2, env.home);
            let overrides = calls[0]
                .1
                .windows(2)
                .filter(|args| args[0] == "-c")
                .map(|args| args[1].as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let overrides: DocumentMut = overrides.parse().unwrap();
            assert_eq!(
                overrides["marketplaces"]["test.mp"]["source"].as_str(),
                Some(expected)
            );
        }
        assert!(!env.home.join(".codex/config.toml").exists());
    }

    #[test]
    fn a_failed_project_marketplace_download_leaves_configuration_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let env = Env::new(dir.path().join("home"), dir.path().join("project"));
        let steps =
            project_install_steps("mattpocock/skills", "mattpocock", "mattpocock-skills", &env);
        let runner = FakeRunner::new(|_| {}).failing("codex");

        assert!(crate::action::execute(&steps, &runner).is_err());
        assert!(!env.project.join(".codex/config.toml").exists());
        assert!(!env.home.join(".codex/config.toml").exists());
    }

    fn runs(plan: &[Action]) -> Vec<Vec<String>> {
        plan.iter()
            .filter_map(|a| match a {
                Action::Run { program, args, .. } => {
                    Some([program.clone()].into_iter().chain(args.clone()).collect())
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn bundled_predicate() {
        assert!(is_bundled("openai-bundled", Some("local")));
        assert!(!is_bundled("openai-bundled", Some("git")));
        assert!(!is_bundled("mine", Some("local")));
        assert!(!is_bundled("openai-x", None));
    }

    #[test]
    fn display_source_strips_github() {
        assert_eq!(display_source("https://github.com/a/b.git"), "a/b");
        assert_eq!(display_source("https://github.com/a/b"), "a/b");
        assert_eq!(display_source("/local/path"), "/local/path");
    }

    #[test]
    fn global_items_remove_with_codex_and_project_items_with_file_edits() {
        let doc = "[plugins.\"p@mp\"]\nenabled = true\n"
            .parse::<DocumentMut>()
            .unwrap();
        let config = Path::new("/h/.codex/config.toml");
        let cache = Path::new("/h/.codex/plugins/cache");
        let cx = |scope| Cx {
            scope,
            config,
            cache,
            marketplaces: &[],
            doc: &doc,
        };
        let global = plugin(&cx(Scope::Global), "p@mp", false).unwrap();
        assert_eq!(
            runs(&global.removal),
            [["codex", "plugin", "remove", "p@mp"]]
        );
        let project = plugin(&cx(Scope::Project), "p@mp", false).unwrap();
        assert!(runs(&project.removal).is_empty());
        let flag = plugin(&cx(Scope::Global), "-x@mp", false).unwrap();
        assert!(runs(&flag.removal).is_empty());
    }
}
