use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde_json::Value;

use super::AgentAdapter;
use crate::action::{Action, Edit};
use crate::env::Env;
use crate::model::{Agent, Augmentation, Kind, Scope};

pub struct Claude;

impl AgentAdapter for Claude {
    fn agent(&self) -> Agent {
        Agent::Claude
    }

    fn skill_dirs(&self, env: &Env, scope: Scope) -> Vec<PathBuf> {
        vec![env.root(scope).join(".claude/skills")]
    }

    fn hook_files(&self, env: &Env, scope: Scope) -> Vec<PathBuf> {
        settings_files(env.root(scope), scope)
    }

    fn discover(&self, env: &Env, scope: Scope) -> anyhow::Result<Vec<Augmentation>> {
        let installs = installs_from(&read_json(&installed_file(&env.home))?);
        let markets = marketplaces_from(&read_json(&marketplaces_file(&env.home))?);
        let mut out = plugins(env, scope, &installs, &markets);
        if scope == Scope::Global {
            out.extend(markets.iter().map(|m| marketplace(&env.home, m, &installs)));
        }
        Ok(out)
    }
}

/// One entry of `installed_plugins.json` `plugins[id][]`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Install {
    id: String,
    /// `None` for `"scope": "user"` (global) entries, else the entry's `projectPath`.
    project: Option<PathBuf>,
    install_path: Option<PathBuf>,
}

/// One entry of `known_marketplaces.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Marketplace {
    name: String,
    source: Option<String>,
    install_location: String,
}

fn installed_file(home: &Path) -> PathBuf {
    home.join(".claude/plugins/installed_plugins.json")
}

fn marketplaces_file(home: &Path) -> PathBuf {
    home.join(".claude/plugins/known_marketplaces.json")
}

fn settings_files(root: &Path, scope: Scope) -> Vec<PathBuf> {
    let dir = root.join(".claude");
    match scope {
        Scope::Global => vec![dir.join("settings.json")],
        Scope::Project => vec![dir.join("settings.json"), dir.join("settings.local.json")],
    }
}

/// Missing file = `Null` (no entries); malformed JSON is an error.
fn read_json(path: &Path) -> anyhow::Result<Value> {
    match fs::read_to_string(path) {
        Ok(text) => {
            serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
        }
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(Value::Null),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

fn str_at<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

fn installs_from(json: &Value) -> Vec<Install> {
    let Some(plugins) = json.get("plugins").and_then(Value::as_object) else {
        return vec![];
    };
    plugins
        .iter()
        .flat_map(|(id, entries)| {
            entries
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(move |e| {
                    let project = match str_at(e, "scope") {
                        Some("user") => None,
                        _ => Some(PathBuf::from(str_at(e, "projectPath")?)),
                    };
                    Some(Install {
                        id: id.clone(),
                        project,
                        install_path: str_at(e, "installPath").map(PathBuf::from),
                    })
                })
        })
        .collect()
}

fn marketplaces_from(json: &Value) -> Vec<Marketplace> {
    let Some(markets) = json.as_object() else {
        return vec![];
    };
    markets
        .iter()
        .map(|(name, m)| {
            let src = m.get("source").unwrap_or(&Value::Null);
            Marketplace {
                name: name.clone(),
                source: ["repo", "url", "path"]
                    .iter()
                    .find_map(|k| str_at(src, k))
                    .map(String::from),
                install_location: str_at(m, "installLocation").unwrap_or_default().into(),
            }
        })
        .collect()
}

/// Marketplace part of a `name@mp` plugin id.
fn mp_of(id: &str) -> &str {
    id.rsplit_once('@').map_or(id, |(_, mp)| mp)
}

fn same_path(a: &Path, b: &Path) -> bool {
    a == b || matches!((a.canonicalize(), b.canonicalize()), (Ok(x), Ok(y)) if x == y)
}

fn dir_entries(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.path()).collect())
        .unwrap_or_default()
}

fn plugin_children(root: &Path) -> Vec<String> {
    let mut out: Vec<String> = dir_entries(&root.join("skills"))
        .into_iter()
        .filter(|p| p.join("SKILL.md").is_file())
        .filter_map(|p| Some(format!("skill: {}", p.file_name()?.to_str()?)))
        .collect();
    for (dir, label) in [("agents", "agent"), ("commands", "command")] {
        out.extend(
            dir_entries(&root.join(dir))
                .into_iter()
                .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "md"))
                .filter_map(|p| Some(format!("{label}: {}", p.file_stem()?.to_str()?))),
        );
    }
    if root.join("hooks/hooks.json").is_file() {
        out.push("hooks".into());
    }
    out.sort();
    out
}

fn plugin_removal(home: &Path, install: &Install) -> Vec<Action> {
    let settings = match &install.project {
        None => settings_files(home, Scope::Global),
        Some(p) => settings_files(p, Scope::Project),
    };
    settings
        .into_iter()
        .map(|file| Action::Edit {
            file,
            edit: Edit::JsonRemoveKey {
                path: vec!["enabledPlugins".into(), install.id.clone()],
            },
        })
        .chain(std::iter::once(Action::Edit {
            file: installed_file(home),
            edit: Edit::JsonRemovePluginInstall {
                id: install.id.clone(),
                project: install.project.clone(),
            },
        }))
        .collect()
}

/// One plugin augmentation per id among the installs belonging to `scope`.
fn plugins(
    env: &Env,
    scope: Scope,
    installs: &[Install],
    markets: &[Marketplace],
) -> Vec<Augmentation> {
    let in_scope = |i: &&Install| match (scope, &i.project) {
        (Scope::Global, None) => true,
        (Scope::Project, Some(p)) => same_path(p, &env.project),
        _ => false,
    };
    let mut seen = HashSet::new();
    installs
        .iter()
        .filter(in_scope)
        .filter(|i| seen.insert(i.id.clone()))
        .map(|i| {
            let mp = mp_of(&i.id);
            Augmentation {
                id: format!("plugin:claude:{scope}:{}", i.id),
                name: i.id.clone(),
                kind: Kind::Plugin,
                agents: vec![Agent::Claude],
                scope,
                group: mp.into(),
                source: markets
                    .iter()
                    .find(|m| m.name == mp)
                    .and_then(|m| m.source.clone()),
                children: i
                    .install_path
                    .as_deref()
                    .map(plugin_children)
                    .unwrap_or_default(),
                removal: plugin_removal(&env.home, i),
            }
        })
        .collect()
}

fn marketplace(home: &Path, m: &Marketplace, installs: &[Install]) -> Augmentation {
    let own: Vec<&Install> = installs.iter().filter(|i| mp_of(&i.id) == m.name).collect();
    let children: BTreeSet<String> = own
        .iter()
        .map(|i| i.id.rsplit_once('@').map_or(&*i.id, |(n, _)| n).to_string())
        .collect();
    let mut removal: Vec<Action> = Vec::new();
    for a in own.iter().flat_map(|i| plugin_removal(home, i)) {
        if !removal.contains(&a) {
            removal.push(a);
        }
    }
    removal.push(Action::Edit {
        file: marketplaces_file(home),
        edit: Edit::JsonRemoveKey {
            path: vec![m.name.clone()],
        },
    });
    removal.push(Action::Edit {
        file: home.join(".claude/settings.json"),
        edit: Edit::JsonRemoveKey {
            path: vec!["extraKnownMarketplaces".into(), m.name.clone()],
        },
    });
    if !m.install_location.is_empty() {
        removal.push(Action::RemovePath(PathBuf::from(&m.install_location)));
    }
    Augmentation {
        id: format!("marketplace:claude:{}", m.name),
        name: m.name.clone(),
        kind: Kind::Marketplace,
        agents: vec![Agent::Claude],
        scope: Scope::Global,
        group: m.name.clone(),
        source: m.source.clone(),
        children: children.into_iter().collect(),
        removal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    struct Fixture {
        _tmp: TempDir,
        env: Env,
        cache: PathBuf,
        mp_dir: PathBuf,
    }

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn edit(file: &Path, edit: Edit) -> Action {
        Action::Edit {
            file: file.to_path_buf(),
            edit,
        }
    }

    fn remove_key(file: &Path, path: &[&str]) -> Action {
        edit(
            file,
            Edit::JsonRemoveKey {
                path: path.iter().map(|s| s.to_string()).collect(),
            },
        )
    }

    fn remove_install(file: &Path, id: &str, project: Option<&Path>) -> Action {
        edit(
            file,
            Edit::JsonRemovePluginInstall {
                id: id.into(),
                project: project.map(Path::to_path_buf),
            },
        )
    }

    /// `tools@acme` installed user-scope and in the project (twice: project + local),
    /// `solo@acme` only in another project, `lint@other` user-scope from a url marketplace.
    fn fixture() -> Fixture {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let project = tmp.path().join("proj");
        let other = tmp.path().join("other");
        fs::create_dir_all(&project).unwrap();
        let cache = home.join(".claude/plugins/cache/acme/tools/1.0.0");
        write(&cache.join("skills/alpha/SKILL.md"), "# a");
        write(&cache.join("skills/beta/SKILL.md"), "# b");
        fs::create_dir_all(cache.join("skills/not-a-skill")).unwrap();
        write(&cache.join("agents/reviewer.md"), "x");
        write(&cache.join("agents/notes.txt"), "x");
        write(&cache.join("commands/ship.md"), "x");
        write(&cache.join("hooks/hooks.json"), "{}");
        let mp_dir = home.join(".claude/plugins/marketplaces/acme");
        let installed = json!({
            "version": 2,
            "plugins": {
                "tools@acme": [
                    {"scope": "user", "installPath": cache, "version": "1.0.0"},
                    {"scope": "project", "projectPath": project, "installPath": cache, "version": "1.0.0"},
                    {"scope": "local", "projectPath": project, "installPath": cache, "version": "1.0.0"}
                ],
                "solo@acme": [
                    {"scope": "project", "projectPath": other, "installPath": "/nonexistent/solo"}
                ],
                "lint@other": [
                    {"scope": "user", "installPath": "/nonexistent/lint"},
                    {"scope": "project"}
                ]
            }
        });
        let known = json!({
            "acme": {"source": {"source": "github", "repo": "acme/plugins"}, "installLocation": mp_dir},
            "other": {"source": {"source": "git", "url": "https://example.com/other.git"}, "installLocation": ""}
        });
        write(&installed_file(&home), &installed.to_string());
        write(&marketplaces_file(&home), &known.to_string());
        Fixture {
            _tmp: tmp,
            env: Env { home, project },
            cache,
            mp_dir,
        }
    }

    fn by_id<'a>(items: &'a [Augmentation], id: &str) -> &'a Augmentation {
        items.iter().find(|a| a.id == id).unwrap()
    }

    #[test]
    fn skill_dirs_and_hook_files() {
        let env = Env {
            home: "/h".into(),
            project: "/p".into(),
        };
        assert_eq!(Claude.agent(), Agent::Claude);
        assert_eq!(
            Claude.skill_dirs(&env, Scope::Global),
            vec![PathBuf::from("/h/.claude/skills")]
        );
        assert_eq!(
            Claude.skill_dirs(&env, Scope::Project),
            vec![PathBuf::from("/p/.claude/skills")]
        );
        assert_eq!(
            Claude.hook_files(&env, Scope::Global),
            vec![PathBuf::from("/h/.claude/settings.json")]
        );
        assert_eq!(
            Claude.hook_files(&env, Scope::Project),
            vec![
                PathBuf::from("/p/.claude/settings.json"),
                PathBuf::from("/p/.claude/settings.local.json")
            ]
        );
    }

    #[test]
    fn global_plugins() {
        let f = fixture();
        let items = Claude.discover(&f.env, Scope::Global).unwrap();
        let plugins: Vec<&str> = items
            .iter()
            .filter(|a| a.kind == Kind::Plugin)
            .map(|a| a.id.as_str())
            .collect();
        assert_eq!(
            plugins,
            [
                "plugin:claude:global:tools@acme",
                "plugin:claude:global:lint@other"
            ]
        );

        let tools = by_id(&items, "plugin:claude:global:tools@acme");
        assert_eq!(tools.name, "tools@acme");
        assert_eq!(tools.group, "acme");
        assert_eq!(tools.scope, Scope::Global);
        assert_eq!(tools.agents, [Agent::Claude]);
        assert_eq!(tools.source.as_deref(), Some("acme/plugins"));
        // 2 skills + 1 agent + 1 command + hooks
        assert_eq!(tools.children.len(), 5);
        let mut sorted = tools.children.clone();
        sorted.sort();
        assert_eq!(tools.children, sorted);
        let installed = installed_file(&f.env.home);
        assert_eq!(
            tools.removal,
            [
                remove_key(
                    &f.env.home.join(".claude/settings.json"),
                    &["enabledPlugins", "tools@acme"]
                ),
                remove_install(&installed, "tools@acme", None),
            ]
        );

        let lint = by_id(&items, "plugin:claude:global:lint@other");
        assert_eq!(
            lint.source.as_deref(),
            Some("https://example.com/other.git")
        );
        assert!(lint.children.is_empty());
        assert!(f.cache.is_dir());
    }

    #[test]
    fn project_plugins_one_per_id() {
        let f = fixture();
        let items = Claude.discover(&f.env, Scope::Project).unwrap();
        assert_eq!(items.len(), 1);
        let tools = &items[0];
        assert_eq!(tools.id, "plugin:claude:project:tools@acme");
        assert_eq!(tools.kind, Kind::Plugin);
        assert_eq!(tools.scope, Scope::Project);
        assert_eq!(tools.children.len(), 5);
        let p = &f.env.project;
        assert_eq!(
            tools.removal,
            [
                remove_key(
                    &p.join(".claude/settings.json"),
                    &["enabledPlugins", "tools@acme"]
                ),
                remove_key(
                    &p.join(".claude/settings.local.json"),
                    &["enabledPlugins", "tools@acme"]
                ),
                remove_install(&installed_file(&f.env.home), "tools@acme", Some(p)),
            ]
        );
    }

    #[test]
    fn project_path_matches_after_canonicalizing() {
        let f = fixture();
        fs::create_dir_all(f.env.project.join("sub")).unwrap();
        let env = Env {
            home: f.env.home.clone(),
            project: f.env.project.join("sub/.."),
        };
        let items = Claude.discover(&env, Scope::Project).unwrap();
        assert_eq!(
            items.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(),
            ["plugin:claude:project:tools@acme"]
        );
    }

    #[test]
    fn marketplaces() {
        let f = fixture();
        let items = Claude.discover(&f.env, Scope::Global).unwrap();
        let home = &f.env.home;
        let installed = installed_file(home);
        let known = marketplaces_file(home);
        let global_settings = home.join(".claude/settings.json");

        let acme = by_id(&items, "marketplace:claude:acme");
        assert_eq!(acme.kind, Kind::Marketplace);
        assert_eq!(acme.name, "acme");
        assert_eq!(acme.group, "acme");
        assert_eq!(acme.source.as_deref(), Some("acme/plugins"));
        assert_eq!(acme.children, ["solo", "tools"]);
        let p = &f.env.project;
        let other = f.env.project.parent().unwrap().join("other");
        assert_eq!(
            acme.removal,
            [
                remove_key(&global_settings, &["enabledPlugins", "tools@acme"]),
                remove_install(&installed, "tools@acme", None),
                remove_key(
                    &p.join(".claude/settings.json"),
                    &["enabledPlugins", "tools@acme"]
                ),
                remove_key(
                    &p.join(".claude/settings.local.json"),
                    &["enabledPlugins", "tools@acme"]
                ),
                remove_install(&installed, "tools@acme", Some(p)),
                remove_key(
                    &other.join(".claude/settings.json"),
                    &["enabledPlugins", "solo@acme"]
                ),
                remove_key(
                    &other.join(".claude/settings.local.json"),
                    &["enabledPlugins", "solo@acme"]
                ),
                remove_install(&installed, "solo@acme", Some(&other)),
                remove_key(&known, &["acme"]),
                remove_key(&global_settings, &["extraKnownMarketplaces", "acme"]),
                Action::RemovePath(f.mp_dir.clone()),
            ]
        );

        let other_mp = by_id(&items, "marketplace:claude:other");
        assert_eq!(other_mp.children, ["lint"]);
        assert_eq!(
            other_mp.removal,
            [
                remove_key(&global_settings, &["enabledPlugins", "lint@other"]),
                remove_install(&installed, "lint@other", None),
                remove_key(&known, &["other"]),
                remove_key(&global_settings, &["extraKnownMarketplaces", "other"]),
            ]
        );
    }

    #[test]
    fn missing_files_are_empty() {
        let tmp = TempDir::new().unwrap();
        let env = Env {
            home: tmp.path().join("home"),
            project: tmp.path().join("proj"),
        };
        assert!(Claude.discover(&env, Scope::Global).unwrap().is_empty());
        assert!(Claude.discover(&env, Scope::Project).unwrap().is_empty());
    }

    #[test]
    fn malformed_json_is_err() {
        for file in [installed_file, marketplaces_file] {
            let f = fixture();
            write(&file(&f.env.home), "{not json");
            assert!(Claude.discover(&f.env, Scope::Global).is_err());
            assert!(Claude.discover(&f.env, Scope::Project).is_err());
        }
    }

    #[test]
    fn unreadable_path_is_err() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("x.json")).unwrap();
        assert!(read_json(&tmp.path().join("x.json")).is_err());
    }

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
        assert_eq!(mp_of("bare"), "bare");
        assert_eq!(mp_of("a@b@c"), "c");
        assert!(installs_from(&json!({"plugins": {"x@y": "nope"}})).is_empty());
        assert!(installs_from(&json!({})).is_empty());
    }
}
