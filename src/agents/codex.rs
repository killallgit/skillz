use std::path::{Path, PathBuf};

use anyhow::Context;
use toml_edit::{DocumentMut, Item};

use super::AgentAdapter;
use crate::action::{Action, Edit, RemovalTarget};
use crate::env::Env;
use crate::model::{Agent, Augmentation, Kind, SKILL_CHILD, Scope};
use crate::npx_skills;

pub struct Codex;

impl AgentAdapter for Codex {
    fn agent(&self) -> Agent {
        Agent::Codex
    }

    fn skill_dirs(&self, env: &Env, scope: Scope) -> Vec<PathBuf> {
        match scope {
            Scope::Global => vec![
                env.home.join(".codex/skills"),
                npx_skills::store_dir(env, scope),
            ],
            Scope::Project => vec![
                npx_skills::store_dir(env, scope),
                env.project.join(".codex/skills"),
            ],
        }
    }

    fn hook_files(&self, env: &Env, scope: Scope) -> Vec<PathBuf> {
        let dir = env.root(scope).join(".codex");
        vec![dir.join("hooks.json"), dir.join("config.toml")]
    }

    fn discover(&self, env: &Env, scope: Scope) -> anyhow::Result<Vec<Augmentation>> {
        let config = env.root(scope).join(".codex/config.toml");
        let Some(doc) = read_config(&config)? else {
            return Ok(vec![]);
        };
        let cache = env.home.join(".codex/plugins/cache");
        let in_project = project_plugin_ids(env, &config);
        let in_use = |pick: &dyn Fn(&str) -> bool| {
            in_project
                .as_ref()
                .is_none_or(|ids| ids.iter().any(|id| pick(id)))
        };
        let marketplaces = marketplaces(&doc);
        let plugins: Vec<Augmentation> = plugin_ids(&doc)
            .into_iter()
            .filter(|id| {
                !marketplaces
                    .iter()
                    .any(|m| m.bundled && m.name == split_id(id).1)
            })
            .map(|id| {
                let keep_cache = in_use(&|other| other == id);
                plugin(&id, scope, &config, &cache, &marketplaces, &doc, keep_cache)
            })
            .collect();
        let mps = marketplaces
            .iter()
            .filter(|m| !m.bundled)
            .map(|m| {
                let keep_cache = in_use(&|other| split_id(other).1 == m.name);
                marketplace(m, scope, &config, &cache, &plugins, keep_cache)
            })
            .collect::<Vec<_>>();
        Ok(mps.into_iter().chain(plugins).collect())
    }
}

struct Marketplace {
    name: String,
    source: Option<String>,
    bundled: bool,
}

fn read_config(path: &Path) -> anyhow::Result<Option<DocumentMut>> {
    match std::fs::read_to_string(path) {
        Ok(text) => text
            .parse::<DocumentMut>()
            .map(Some)
            .with_context(|| format!("parsing {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

fn table_keys<'a>(doc: &'a DocumentMut, key: &str) -> Vec<(&'a str, &'a Item)> {
    doc.get(key)
        .and_then(Item::as_table_like)
        .map(|t| t.iter().collect())
        .unwrap_or_default()
}

fn str_field<'a>(item: &'a Item, key: &str) -> Option<&'a str> {
    item.get(key).and_then(Item::as_str)
}

fn marketplaces(doc: &DocumentMut) -> Vec<Marketplace> {
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

/// Plugin ids the current project's config lists (enabled or not), so a global removal keeps
/// the shared cache they load from; `None` (keep every cache) when it cannot be read. Empty
/// when that config is `config` itself (project scope, or run from home).
// shortcut: other projects' configs are not visible, so their use is not protected; upgrade
// if Codex ever records which projects use a cached plugin.
fn project_plugin_ids(env: &Env, config: &Path) -> Option<Vec<String>> {
    let project = env.project.join(".codex/config.toml");
    if std::fs::canonicalize(&project).ok() == std::fs::canonicalize(config).ok() {
        return Some(vec![]);
    }
    let doc = read_config(&project).ok()?;
    Some(doc.as_ref().map(plugin_ids).unwrap_or_default())
}

// Ids without `@mp` are not a shape Codex installs; skip rather than guess a cache path.
fn plugin_ids(doc: &DocumentMut) -> Vec<String> {
    table_keys(doc, "plugins")
        .into_iter()
        .map(|(id, _)| id.to_string())
        .filter(|id| id.contains('@'))
        .collect()
}

/// `[hooks.state."<id>:..."]` trust entries Codex keeps for a plugin's hooks.
fn hook_state_keys(doc: &DocumentMut, id: &str) -> Vec<String> {
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
fn split_id(id: &str) -> (&str, &str) {
    id.rsplit_once('@').unwrap_or((id, ""))
}

fn plugin(
    id: &str,
    scope: Scope,
    config: &Path,
    cache: &Path,
    marketplaces: &[Marketplace],
    doc: &DocumentMut,
    keep_cache: bool,
) -> Augmentation {
    let (name, mp) = split_id(id);
    let plugin_cache = cache.join(mp).join(name);
    let mut removal = vec![toml_remove(config, &["plugins", id])];
    removal.extend(
        hook_state_keys(doc, id)
            .iter()
            .map(|k| toml_remove(config, &["hooks", "state", k])),
    );
    if scope == Scope::Global && !keep_cache {
        removal.extend(RemovalTarget::new(cache, [mp, name]).map(Action::RemovePath));
    }
    Augmentation {
        id: format!("plugin:codex:{scope}:{id}"),
        name: id.to_string(),
        kind: Kind::Plugin,
        agents: vec![Agent::Codex],
        scope,
        group: mp.to_string(),
        source: marketplaces
            .iter()
            .find(|m| m.name == mp)
            .and_then(|m| m.source.clone()),
        children: plugin_children(&plugin_cache),
        removal,
    }
}

fn marketplace(
    m: &Marketplace,
    scope: Scope,
    config: &Path,
    cache: &Path,
    plugins: &[Augmentation],
    keep_cache: bool,
) -> Augmentation {
    let own: Vec<&Augmentation> = plugins.iter().filter(|p| p.group == m.name).collect();
    let mut removal: Vec<Action> = own.iter().flat_map(|p| p.removal.clone()).collect();
    removal.push(toml_remove(config, &["marketplaces", &m.name]));
    if scope == Scope::Global && !keep_cache {
        removal.extend(RemovalTarget::new(cache, [&m.name]).map(Action::RemovePath));
    }
    Augmentation {
        id: format!("marketplace:codex:{scope}:{}", m.name),
        name: m.name.clone(),
        kind: Kind::Marketplace,
        agents: vec![Agent::Codex],
        scope,
        group: m.name.clone(),
        source: m.source.clone(),
        children: own.iter().map(|p| p.name.clone()).collect(),
        removal,
    }
}

fn toml_remove(config: &Path, path: &[&str]) -> Action {
    Action::Edit {
        file: config.to_path_buf(),
        edit: Edit::TomlRemoveKey {
            path: path.iter().map(|k| k.to_string()).collect(),
        },
    }
}

/// Skills and hooks found in any version dir of a cached plugin.
fn plugin_children(plugin_cache: &Path) -> Vec<String> {
    let base = glob::Pattern::escape(&plugin_cache.to_string_lossy());
    let matches = |pattern: &str| -> Vec<PathBuf> {
        glob::glob(&format!("{base}/{pattern}"))
            .map(|paths| paths.flatten().collect())
            .unwrap_or_default()
    };
    let mut skills: Vec<String> = matches("*/skills/*/SKILL.md")
        .iter()
        .filter_map(|p| p.parent()?.file_name()?.to_str())
        .map(|dir| format!("{SKILL_CHILD}{dir}"))
        .collect();
    skills.sort();
    skills.dedup();
    let has_hooks =
        !matches("*/hooks/hooks.json").is_empty() || !matches("*/hooks.json").is_empty();
    skills
        .into_iter()
        .chain(has_hooks.then(|| "hooks".to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const CONFIG: &str = r#"
model = "gpt-5"

[desktop]
theme = "dark"

[features]
web_search = true

[marketplaces.openai-bundled]
source_type = "local"
source = "/Applications/Codex.app/plugins"

[marketplaces.killall]
source_type = "git"
source = "https://github.com/killallgit/killall-skills.git"

[marketplaces.localdev]
source_type = "local"
source = "/src/my-plugins"

[plugins."browser@openai-bundled"]
enabled = true

[plugins."review@killall"]
enabled = true

[plugins."commit@killall"]
enabled = false

[hooks.state."review@killall:hooks/hooks.json:session_start:0:0"]
trusted_hash = "abc"
enabled = true

[hooks.state."review@killall:hooks/hooks.json:stop:0:0"]
enabled = false

[hooks.state."review@killall-old:hooks/hooks.json:stop:0:0"]
enabled = true
"#;

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn setup() -> (tempfile::TempDir, Env) {
        let dir = tempfile::tempdir().unwrap();
        let env = Env {
            home: dir.path().join("home"),
            project: dir.path().join("proj"),
        };
        write(&env.home.join(".codex/config.toml"), CONFIG);
        let cache = env.home.join(".codex/plugins/cache/killall");
        write(&cache.join("review/1.0.0/skills/lint/SKILL.md"), "x");
        write(&cache.join("review/1.0.0/skills/diff/SKILL.md"), "x");
        write(&cache.join("review/1.1.0/skills/lint/SKILL.md"), "x");
        write(&cache.join("review/1.1.0/hooks/hooks.json"), "{}");
        write(&cache.join("commit/0.1.0/hooks.json"), "{}");
        (dir, env)
    }

    fn find<'a>(items: &'a [Augmentation], id: &str) -> &'a Augmentation {
        items.iter().find(|a| a.id == id).unwrap()
    }

    fn rm_state(file: &Path, key: &str) -> Action {
        Action::Edit {
            file: file.to_path_buf(),
            edit: Edit::TomlRemoveKey {
                path: vec!["hooks".into(), "state".into(), key.into()],
            },
        }
    }

    const REVIEW_START: &str = "review@killall:hooks/hooks.json:session_start:0:0";
    const REVIEW_STOP: &str = "review@killall:hooks/hooks.json:stop:0:0";

    fn rm(root: &Path, segments: &[&str]) -> Action {
        Action::RemovePath(RemovalTarget::new(root, segments).unwrap())
    }

    fn rm_key(file: &Path, table: &str, key: &str) -> Action {
        Action::Edit {
            file: file.to_path_buf(),
            edit: Edit::TomlRemoveKey {
                path: vec![table.into(), key.into()],
            },
        }
    }

    #[test]
    fn paths_per_scope() {
        let env = Env {
            home: "/h".into(),
            project: "/p".into(),
        };
        assert_eq!(Codex.agent(), Agent::Codex);
        assert_eq!(
            Codex.skill_dirs(&env, Scope::Global),
            vec![
                PathBuf::from("/h/.codex/skills"),
                "/h/.agents/skills".into()
            ]
        );
        assert_eq!(
            Codex.skill_dirs(&env, Scope::Project),
            vec![
                PathBuf::from("/p/.agents/skills"),
                "/p/.codex/skills".into()
            ]
        );
        assert_eq!(
            Codex.hook_files(&env, Scope::Global),
            vec![
                PathBuf::from("/h/.codex/hooks.json"),
                PathBuf::from("/h/.codex/config.toml")
            ]
        );
        assert_eq!(
            Codex.hook_files(&env, Scope::Project),
            vec![
                PathBuf::from("/p/.codex/hooks.json"),
                PathBuf::from("/p/.codex/config.toml")
            ]
        );
    }

    #[test]
    fn global_discovery_skips_bundled() {
        let (_d, env) = setup();
        let items = Codex.discover(&env, Scope::Global).unwrap();
        let mut ids: Vec<&str> = items.iter().map(|a| a.id.as_str()).collect();
        ids.sort();
        assert_eq!(
            ids,
            vec![
                "marketplace:codex:global:killall",
                "marketplace:codex:global:localdev",
                "plugin:codex:global:commit@killall",
                "plugin:codex:global:review@killall",
            ]
        );
    }

    #[test]
    fn global_plugin_fields_and_removal() {
        let (_d, env) = setup();
        let items = Codex.discover(&env, Scope::Global).unwrap();
        let config = env.home.join(".codex/config.toml");
        let cache = env.home.join(".codex/plugins/cache/killall");

        let review = find(&items, "plugin:codex:global:review@killall");
        assert_eq!(review.kind, Kind::Plugin);
        assert_eq!(review.name, "review@killall");
        assert_eq!(review.group, "killall");
        assert_eq!(review.agents, vec![Agent::Codex]);
        assert_eq!(review.scope, Scope::Global);
        assert_eq!(review.source.as_deref(), Some("killallgit/killall-skills"));
        assert_eq!(review.children.len(), 3);
        assert_eq!(
            review.removal,
            vec![
                rm_key(&config, "plugins", "review@killall"),
                rm_state(&config, REVIEW_START),
                rm_state(&config, REVIEW_STOP),
                rm(cache.parent().unwrap(), &["killall", "review"]),
            ]
        );

        let commit = find(&items, "plugin:codex:global:commit@killall");
        assert_eq!(commit.children.len(), 1);
    }

    #[test]
    fn global_marketplace_fields_and_removal() {
        let (_d, env) = setup();
        let items = Codex.discover(&env, Scope::Global).unwrap();
        let config = env.home.join(".codex/config.toml");
        let cache = env.home.join(".codex/plugins/cache");

        let mp = find(&items, "marketplace:codex:global:killall");
        assert_eq!(mp.kind, Kind::Marketplace);
        assert_eq!(mp.name, "killall");
        assert_eq!(mp.group, "killall");
        assert_eq!(mp.source.as_deref(), Some("killallgit/killall-skills"));
        assert_eq!(mp.children.len(), 2);
        assert_eq!(
            mp.removal,
            vec![
                rm_key(&config, "plugins", "review@killall"),
                rm_state(&config, REVIEW_START),
                rm_state(&config, REVIEW_STOP),
                rm(&cache, &["killall", "review"]),
                rm_key(&config, "plugins", "commit@killall"),
                rm(&cache, &["killall", "commit"]),
                rm_key(&config, "marketplaces", "killall"),
                rm(&cache, &["killall"]),
            ]
        );

        let local = find(&items, "marketplace:codex:global:localdev");
        assert_eq!(local.source.as_deref(), Some("/src/my-plugins"));
        assert!(local.children.is_empty());
        assert_eq!(
            local.removal,
            vec![
                rm_key(&config, "marketplaces", "localdev"),
                rm(&cache, &["localdev"]),
            ]
        );
    }

    #[test]
    fn global_removal_keeps_cache_the_project_still_enables() {
        let (_d, env) = setup();
        let config = env.home.join(".codex/config.toml");
        let cache = env.home.join(".codex/plugins/cache");
        let project_config = env.project.join(".codex/config.toml");
        write(
            &project_config,
            "[plugins.\"review@killall\"]\nenabled = true\n",
        );
        let review = vec![
            rm_key(&config, "plugins", "review@killall"),
            rm_state(&config, REVIEW_START),
            rm_state(&config, REVIEW_STOP),
        ];
        let commit = vec![
            rm_key(&config, "plugins", "commit@killall"),
            rm(&cache, &["killall", "commit"]),
        ];
        let items = Codex.discover(&env, Scope::Global).unwrap();
        assert_eq!(
            find(&items, "plugin:codex:global:review@killall").removal,
            review
        );
        assert_eq!(
            find(&items, "plugin:codex:global:commit@killall").removal,
            commit
        );
        assert_eq!(
            find(&items, "marketplace:codex:global:killall").removal,
            [
                review,
                commit,
                vec![rm_key(&config, "marketplaces", "killall")]
            ]
            .concat()
        );

        // Unreadable project config: keep every cache rather than guess.
        write(&project_config, "[plugins\n");
        let items = Codex.discover(&env, Scope::Global).unwrap();
        assert!(
            items
                .iter()
                .flat_map(|a| &a.removal)
                .all(|a| matches!(a, Action::Edit { .. }))
        );

        // Run from home: the "project" config is the global one and protects nothing.
        let home = Env {
            home: env.home.clone(),
            project: env.home.clone(),
        };
        let items = Codex.discover(&home, Scope::Global).unwrap();
        assert!(
            find(&items, "plugin:codex:global:review@killall")
                .removal
                .contains(&rm(&cache, &["killall", "review"]))
        );
    }

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

    #[test]
    fn unsafe_names_never_become_paths() {
        let (_d, env) = setup();
        let config = env.home.join(".codex/config.toml");
        write(
            &config,
            r#"
[marketplaces.".."]
source_type = "local"
source = "x"

[marketplaces."a/b"]
source_type = "local"
source = "y"

[plugins.killall]
enabled = true

[plugins."x@.."]
enabled = true

[plugins.'..\evil@ok']
enabled = true

[plugins."p@"]
enabled = true
"#,
        );
        let items = Codex.discover(&env, Scope::Global).unwrap();
        let mut ids: Vec<&str> = items.iter().map(|a| a.id.as_str()).collect();
        ids.sort();
        assert_eq!(
            ids,
            vec![
                "marketplace:codex:global:..",
                "marketplace:codex:global:a/b",
                r"plugin:codex:global:..\evil@ok",
                "plugin:codex:global:p@",
                "plugin:codex:global:x@..",
            ]
        );
        assert_eq!(
            find(&items, "marketplace:codex:global:..").removal,
            vec![
                rm_key(&config, "plugins", "x@.."),
                rm_key(&config, "marketplaces", ".."),
            ]
        );
        assert_eq!(
            find(&items, "marketplace:codex:global:a/b").removal,
            vec![rm_key(&config, "marketplaces", "a/b")]
        );
        assert_eq!(
            find(&items, r"plugin:codex:global:..\evil@ok").removal,
            vec![rm_key(&config, "plugins", r"..\evil@ok")]
        );
        assert_eq!(
            find(&items, "plugin:codex:global:p@").removal,
            vec![rm_key(&config, "plugins", "p@")]
        );
    }

    #[test]
    fn project_scope_only_edits_toml() {
        let (_d, env) = setup();
        let config = env.project.join(".codex/config.toml");
        write(
            &config,
            r#"
[plugins."review@killall"]
enabled = true

[plugins."solo@unknown"]
enabled = true
"#,
        );
        let items = Codex.discover(&env, Scope::Project).unwrap();
        assert_eq!(items.len(), 2);

        let review = find(&items, "plugin:codex:project:review@killall");
        assert_eq!(review.source, None);
        assert_eq!(review.children.len(), 3);
        assert_eq!(
            review.removal,
            vec![rm_key(&config, "plugins", "review@killall")]
        );

        let solo = find(&items, "plugin:codex:project:solo@unknown");
        assert_eq!(solo.group, "unknown");
        assert!(solo.children.is_empty());
    }

    #[test]
    fn project_marketplace_removal_has_no_cache_path() {
        let (_d, env) = setup();
        let config = env.project.join(".codex/config.toml");
        write(
            &config,
            "[marketplaces.team]\nsource_type = \"git\"\nsource = \"git@example.com:t/p.git\"\n",
        );
        let items = Codex.discover(&env, Scope::Project).unwrap();
        let mp = find(&items, "marketplace:codex:project:team");
        assert_eq!(mp.source.as_deref(), Some("git@example.com:t/p.git"));
        assert_eq!(mp.removal, vec![rm_key(&config, "marketplaces", "team")]);
    }

    #[test]
    fn missing_config_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let env = Env {
            home: dir.path().into(),
            project: dir.path().into(),
        };
        assert!(Codex.discover(&env, Scope::Global).unwrap().is_empty());
        assert!(Codex.discover(&env, Scope::Project).unwrap().is_empty());
    }

    #[test]
    fn malformed_config_is_err() {
        let (_d, env) = setup();
        write(
            &env.project.join(".codex/config.toml"),
            "[plugins\nbroken = ",
        );
        assert!(Codex.discover(&env, Scope::Project).is_err());
    }

    #[test]
    fn unreadable_config_is_err() {
        let (_d, env) = setup();
        fs::create_dir_all(env.project.join(".codex/config.toml")).unwrap();
        assert!(Codex.discover(&env, Scope::Project).is_err());
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
}
