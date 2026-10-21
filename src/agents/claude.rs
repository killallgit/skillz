use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde_json::Value;

use super::{AgentAdapter, PluginInstall, PluginOutcome, claude_marketplace};
use crate::action::{Action, Edit, RemovalTarget};
use crate::env::Env;
use crate::model::{Agent, Augmentation, Installer, Kind, SKILL_CHILD, Scope};
use crate::runner::CommandRunner;

pub struct Claude;

impl AgentAdapter for Claude {
    fn agent(&self) -> Agent {
        Agent::Claude
    }

    fn npx_id(&self) -> &'static str {
        "claude-code"
    }

    fn home_dir_names(&self) -> &'static [&'static str] {
        &["claude"]
    }

    fn plugin_root_var(&self) -> Option<&'static str> {
        Some("CLAUDE_PLUGIN_ROOT")
    }

    fn skill_dirs(&self, env: &Env, scope: Scope) -> Vec<PathBuf> {
        vec![match scope {
            Scope::Global => env.claude_dir().join("skills"),
            Scope::Project => env.project.join(".claude/skills"),
        }]
    }

    fn hook_files(&self, env: &Env, scope: Scope) -> Vec<PathBuf> {
        settings_files(env, scope)
    }

    fn discover(&self, env: &Env, scope: Scope) -> anyhow::Result<Vec<Augmentation>> {
        let installs = installs_from(&read_json(&installed_file(&env.claude_dir()))?);
        let markets = marketplaces_from(&read_json(&marketplaces_file(&env.claude_dir()))?);
        let mut out = plugins(env, scope, &installs, &markets);
        if scope == Scope::Global {
            out.extend(
                markets
                    .iter()
                    .map(|m| marketplace(&env.claude_dir(), m, &installs)),
            );
        }
        Ok(out)
    }

    /// `claude -p "/context"` prints, with no model call, a table of every skill and custom
    /// agent with its tokens. Without the `claude` binary, loose skills are estimated.
    fn context_costs(
        &self,
        env: &Env,
        runner: &dyn CommandRunner,
    ) -> anyhow::Result<Vec<(String, u32)>> {
        let args = ["-p", "/context"].map(String::from);
        match runner.capture("claude", &args, &env.project) {
            Ok(text) => Ok(parse_context(&text)),
            Err(_) => {
                let dirs: Vec<PathBuf> = [Scope::Global, Scope::Project]
                    .iter()
                    .flat_map(|&s| self.skill_dirs(env, s))
                    .collect();
                Ok(crate::cost::estimate_dirs(&dirs))
            }
        }
    }

    fn install_plugins(&self, req: &PluginInstall) -> anyhow::Result<PluginOutcome> {
        claude_marketplace::install_plugins(req)
    }

    /// `~/.claude.json` `skillUsage` (loose skills by name; `plugin:skill` keys are plugin
    /// skills, counted through `pluginUsage`) and `pluginUsage` (`name@mp`, only when
    /// `usageCount > 0`), both with `lastUsedAt` in ms.
    fn usage(&self, env: &Env) -> anyhow::Result<Vec<(String, u64)>> {
        Ok(claude_uses(&read_json(&env.claude_json())?))
    }

    fn tracks(&self, item: &Augmentation) -> bool {
        matches!(item.kind, Kind::Skill | Kind::Plugin)
    }
}

/// One entry of `installed_plugins.json` `plugins[id][]`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Install {
    id: String,
    /// `None` for `"scope": "user"` (global) entries, else the entry's `projectPath`.
    project: Option<PathBuf>,
    /// The entry's `scope` as `claude plugin uninstall --scope` spells it: user, project, local.
    scope: String,
    install_path: Option<PathBuf>,
}

/// One entry of `known_marketplaces.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Marketplace {
    name: String,
    source: Option<String>,
    install_location: String,
}

fn installed_file(claude: &Path) -> PathBuf {
    claude.join("plugins/installed_plugins.json")
}

fn marketplaces_file(claude: &Path) -> PathBuf {
    claude.join("plugins/known_marketplaces.json")
}

fn settings_files(env: &Env, scope: Scope) -> Vec<PathBuf> {
    match scope {
        Scope::Global => global_settings(&env.claude_dir()),
        Scope::Project => project_settings(&env.project),
    }
}

fn global_settings(claude: &Path) -> Vec<PathBuf> {
    vec![claude.join("settings.json")]
}

fn project_settings(root: &Path) -> Vec<PathBuf> {
    let dir = root.join(".claude");
    vec![dir.join("settings.json"), dir.join("settings.local.json")]
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
            let entries = entries.as_array().map(Vec::as_slice).unwrap_or_default();
            entries.iter().filter_map(move |e| {
                let project = match str_at(e, "scope") {
                    Some("user") => None,
                    _ => Some(PathBuf::from(str_at(e, "projectPath")?)),
                };
                Some(Install {
                    id: id.clone(),
                    project,
                    scope: str_at(e, "scope").unwrap_or("user").to_string(),
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
        .filter_map(|p| Some(format!("{SKILL_CHILD}{}", p.file_name()?.to_str()?)))
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

fn plugin_removal(claude: &Path, install: &Install) -> Vec<Action> {
    let settings = match &install.project {
        None => global_settings(claude),
        Some(p) => project_settings(p),
    };
    // `claude plugin uninstall` also drops the cache once no other scope uses the plugin.
    let cwd = install.project.as_deref().unwrap_or(claude);
    let args = [
        "plugin",
        "uninstall",
        &install.id,
        "--scope",
        &install.scope,
    ];
    let native = Action::native("claude", &args, &[&install.id], cwd);
    native
        .into_iter()
        .chain(settings.into_iter().map(|file| Action::Edit {
            file,
            edit: Edit::JsonRemoveKey {
                path: vec!["enabledPlugins".into(), install.id.clone()],
            },
        }))
        .chain(std::iter::once(Action::Edit {
            file: installed_file(claude),
            edit: Edit::JsonRemoveArrayItems {
                path: vec!["plugins".into(), install.id.clone()],
                fields: record_fields(install),
            },
        }))
        .chain(plugin_dirs(claude, &install.id))
        .chain(leftovers(claude, &install.id))
        .collect()
}

/// Files a plugin's own hooks wrote beside Claude's config (`~/.claude/.<name>-active`,
/// `<name>-statusline.sh`, `plugins/store/<name>_<mp>-….json`) and that no uninstall knows
/// about. Matched by name: the plugin name, then `-`, `_` or `.`. Gone with the plugin, and
/// skipped when another installed plugin's longer name claims the file (`killall` vs
/// `killall-agents`). Global installs only; names under 4 chars are too generic to match.
fn leftovers(claude: &Path, id: &str) -> Vec<Action> {
    let Some((name, _)) = id.rsplit_once('@') else {
        return vec![];
    };
    if name.len() < 4 {
        return vec![];
    }
    let claims = |owner: &str, file: &str| {
        let stem = file.trim_start_matches('.');
        stem.strip_prefix(owner)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(['-', '_', '.']))
    };
    let rivals: Vec<String> = read_json(&installed_file(claude))
        .ok()
        .and_then(|j| j.get("plugins").and_then(Value::as_object).cloned())
        .map(|m| {
            m.keys()
                .filter_map(|k| k.rsplit_once('@').map(|(n, _)| n.to_string()))
                .filter(|n| n.len() > name.len() && n.starts_with(name))
                .collect()
        })
        .unwrap_or_default();
    let base = claude.to_path_buf();
    let mut found: Vec<Action> = [base.clone(), base.join("plugins/store")]
        .iter()
        .flat_map(|dir| {
            dir_entries(dir).into_iter().filter_map(|path| {
                let file = path.file_name()?.to_str()?.to_string();
                let mine = claims(name, &file) && !rivals.iter().any(|r| claims(r, &file));
                let target = mine.then(|| RemovalTarget::new(dir, [&file]))??;
                Some(Action::RemovePathIfJsonAbsent {
                    target,
                    file: installed_file(claude),
                    key: vec!["plugins".into(), id.into()],
                })
            })
        })
        .collect();
    found.sort_by_key(|a| format!("{a:?}"));
    found
}

/// Selects the `plugins[id]` records that removing `install` drops: user scope is the global
/// install; any other scope is keyed by its project.
fn record_fields(install: &Install) -> Vec<(String, Value)> {
    match &install.project {
        None => vec![("scope".into(), "user".into())],
        Some(p) => vec![("projectPath".into(), p.to_string_lossy().into())],
    }
}

/// The plugin's cache and data dirs, removed once `plugins[id]` is gone after the edits, so
/// whichever selected items drop its last install record take them; none unless both are
/// valid targets.
fn plugin_dirs(claude: &Path, id: &str) -> Vec<Action> {
    let Some((name, mp)) = id.rsplit_once('@') else {
        return vec![];
    };
    let dir = claude.join("plugins");
    let cache = RemovalTarget::new(&dir.join("cache"), [mp, name]);
    let data = RemovalTarget::new(&dir.join("data"), [format!("{name}-{mp}")]);
    let unused = |target| Action::RemovePathIfJsonAbsent {
        target,
        file: installed_file(claude),
        key: vec!["plugins".into(), id.into()],
    };
    cache
        .zip(data)
        .map_or(vec![], |(c, d)| vec![unused(c), unused(d)])
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
                installer: Installer::ClaudeCli,
                children: i
                    .install_path
                    .as_deref()
                    .map(plugin_children)
                    .unwrap_or_default(),
                removal: plugin_removal(&env.claude_dir(), i),
            }
        })
        .collect()
}

fn marketplace(claude: &Path, m: &Marketplace, installs: &[Install]) -> Augmentation {
    let own: Vec<&Install> = installs.iter().filter(|i| mp_of(&i.id) == m.name).collect();
    let children: BTreeSet<String> = own
        .iter()
        .map(|i| i.id.rsplit_once('@').map_or(&*i.id, |(n, _)| n).to_string())
        .collect();
    let mut removal: Vec<Action> = Vec::new();
    for a in own.iter().flat_map(|i| plugin_removal(claude, i)) {
        if !removal.contains(&a) {
            removal.push(a);
        }
    }
    let args = ["plugin", "marketplace", "remove", &m.name];
    removal.extend(Action::native("claude", &args, &[&m.name], claude));
    removal.push(Action::Edit {
        file: marketplaces_file(claude),
        edit: Edit::JsonRemoveKey {
            path: vec![m.name.clone()],
        },
    });
    removal.push(Action::Edit {
        file: claude.join("settings.json"),
        edit: Edit::JsonRemoveKey {
            path: vec!["extraKnownMarketplaces".into(), m.name.clone()],
        },
    });
    // Only a clone Claude made, as one segment under marketplaces/; a `directory` source's
    // location is the user's checkout.
    let clones = claude.join("plugins/marketplaces");
    let clone = Path::new(&m.install_location)
        .strip_prefix(&clones)
        .ok()
        .and_then(|rest| RemovalTarget::new(&clones, [rest]));
    removal.extend(clone.map(Action::RemovePath));
    Augmentation {
        id: format!("marketplace:claude:{}", m.name),
        name: m.name.clone(),
        kind: Kind::Marketplace,
        agents: vec![Agent::Claude],
        scope: Scope::Global,
        group: m.name.clone(),
        source: m.source.clone(),
        installer: Installer::ClaudeCli,
        children: children.into_iter().collect(),
        removal,
    }
}

fn claude_uses(json: &Value) -> Vec<(String, u64)> {
    let entries = |field: &str| {
        json.get(field)
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter_map(|(k, v)| Some((k.clone(), v.get("lastUsedAt")?.as_u64()?, v)))
    };
    let skills = entries("skillUsage").map(|(k, ms, _)| match k.split_once(':') {
        Some((plugin, skill)) => (format!("pskill:{plugin}:{skill}"), ms),
        None => (format!("skill:{k}"), ms),
    });
    let plugins = entries("pluginUsage")
        .filter(|(_, _, v)| v.get("usageCount").and_then(Value::as_u64) > Some(0))
        .map(|(k, ms, _)| (format!("plugin:{k}"), ms));
    skills.chain(plugins).collect()
}

/// Rows of the `### Skills` and `### Custom Agents` tables of `/context`: loose skills as
/// `skill:<name>`, and everything from a plugin summed under `plugin:<plugin>`. Built-in and
/// claude.ai skills are not ours to remove and are left out.
fn parse_context(text: &str) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    let mut section = "";
    for line in text.lines() {
        if let Some(title) = line.strip_prefix("### ") {
            section = title.trim();
            continue;
        }
        if !matches!(section, "Skills" | "Custom Agents") {
            continue;
        }
        let cells: Vec<&str> = line
            .trim()
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        let [name, source, tokens] = cells[..] else {
            continue;
        };
        let Some(tokens) = context_tokens(tokens) else {
            continue;
        };
        let key = match (source, section) {
            ("User" | "Project", "Skills") => format!("skill:{name}"),
            ("Plugin", "Custom Agents") => {
                format!("plugin:{}", name.split(':').next().unwrap_or(name))
            }
            _ => match source
                .strip_prefix("Plugin (")
                .and_then(|s| s.strip_suffix(')'))
            {
                Some(plugin) => format!("plugin:{plugin}"),
                None => continue,
            },
        };
        out.push((key, tokens));
    }
    out
}

/// `~100`, `1.2k`, `< 20` (counted as 10); header and separator cells are `None`.
fn context_tokens(cell: &str) -> Option<u32> {
    if let Some(small) = cell.strip_prefix('<') {
        return small.trim().parse::<u32>().ok().map(|n| n / 2);
    }
    let cell = cell.trim_start_matches('~');
    match cell.strip_suffix('k') {
        Some(k) => Some((k.parse::<f64>().ok()? * 1000.0).round() as u32),
        None => cell.parse().ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// A removal plan without its native-tool commands.
    fn files(plan: &[Action]) -> Vec<Action> {
        plan.iter()
            .filter(|a| !matches!(a, Action::Run { .. }))
            .cloned()
            .collect()
    }

    use crate::action::apply_edit;
    use crate::model::Installer;
    use serde_json::json;
    use tempfile::TempDir;

    struct Fixture {
        _tmp: TempDir,
        env: Env,
        cache: PathBuf,
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

    fn rm(root: &Path, segments: &[&str]) -> Action {
        Action::RemovePath(RemovalTarget::new(root, segments).unwrap())
    }

    fn remove_key(file: &Path, path: &[&str]) -> Action {
        edit(
            file,
            Edit::JsonRemoveKey {
                path: path.iter().map(|s| s.to_string()).collect(),
            },
        )
    }

    /// `name@mp`'s cache and data dir removals, each waiting for `plugins[id]` to be gone.
    fn dirs(home: &Path, name: &str, mp: &str) -> Vec<Action> {
        let plugins = home.join(".claude/plugins");
        let unused = |root: &str, segments: &[&str]| Action::RemovePathIfJsonAbsent {
            target: RemovalTarget::new(&plugins.join(root), segments).unwrap(),
            file: installed_file(&home.join(".claude")),
            key: vec!["plugins".into(), format!("{name}@{mp}")],
        };
        vec![
            unused("cache", &[mp, name]),
            unused("data", &[&format!("{name}-{mp}")]),
        ]
    }

    fn remove_install(file: &Path, id: &str, project: Option<&Path>) -> Action {
        edit(
            file,
            Edit::JsonRemoveArrayItems {
                path: vec!["plugins".into(), id.into()],
                fields: vec![match project {
                    None => ("scope".into(), json!("user")),
                    Some(p) => ("projectPath".into(), json!(p)),
                }],
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
        write(
            &installed_file(&home.join(".claude")),
            &installed.to_string(),
        );
        write(
            &marketplaces_file(&home.join(".claude")),
            &known.to_string(),
        );
        Fixture {
            _tmp: tmp,
            env: Env {
                home,
                project,
                claude_config: None,
                codex_home: None,
            },
            cache,
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
            claude_config: None,
            codex_home: None,
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
        let installed = installed_file(&f.env.claude_dir());
        assert_eq!(
            files(&tools.removal),
            [
                vec![
                    remove_key(
                        &f.env.home.join(".claude/settings.json"),
                        &["enabledPlugins", "tools@acme"]
                    ),
                    remove_install(&installed, "tools@acme", None),
                ],
                dirs(&f.env.home, "tools", "acme"),
            ]
            .concat()
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
        assert_eq!(tools.installer, Installer::ClaudeCli);
        assert_eq!(tools.scope, Scope::Project);
        assert_eq!(tools.children.len(), 5);
        let p = &f.env.project;
        assert_eq!(
            files(&tools.removal),
            [
                vec![
                    remove_key(
                        &p.join(".claude/settings.json"),
                        &["enabledPlugins", "tools@acme"]
                    ),
                    remove_key(
                        &p.join(".claude/settings.local.json"),
                        &["enabledPlugins", "tools@acme"]
                    ),
                    remove_install(&installed_file(&f.env.claude_dir()), "tools@acme", Some(p)),
                ],
                dirs(&f.env.home, "tools", "acme"),
            ]
            .concat()
        );
    }

    #[test]
    fn project_path_matches_after_canonicalizing() {
        let f = fixture();
        fs::create_dir_all(f.env.project.join("sub")).unwrap();
        let env = Env {
            home: f.env.home.clone(),
            project: f.env.project.join("sub/.."),
            claude_config: None,
            codex_home: None,
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
        let installed = installed_file(&home.join(".claude"));
        let known = marketplaces_file(&home.join(".claude"));
        let global_settings = home.join(".claude/settings.json");

        let acme = by_id(&items, "marketplace:claude:acme");
        assert_eq!(acme.kind, Kind::Marketplace);
        assert_eq!(acme.installer, Installer::ClaudeCli);
        assert_eq!(acme.name, "acme");
        assert_eq!(acme.group, "acme");
        assert_eq!(acme.source.as_deref(), Some("acme/plugins"));
        assert_eq!(acme.children, ["solo", "tools"]);
        let p = &f.env.project;
        let other = f.env.project.parent().unwrap().join("other");
        assert_eq!(
            files(&acme.removal),
            [
                vec![
                    remove_key(&global_settings, &["enabledPlugins", "tools@acme"]),
                    remove_install(&installed, "tools@acme", None),
                ],
                dirs(home, "tools", "acme"),
                vec![
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
                ],
                dirs(home, "solo", "acme"),
                vec![
                    remove_key(&known, &["acme"]),
                    remove_key(&global_settings, &["extraKnownMarketplaces", "acme"]),
                    rm(&home.join(".claude/plugins/marketplaces"), &["acme"]),
                ],
            ]
            .concat()
        );

        let other_mp = by_id(&items, "marketplace:claude:other");
        assert_eq!(other_mp.children, ["lint"]);
        assert_eq!(
            files(&other_mp.removal),
            [
                vec![
                    remove_key(&global_settings, &["enabledPlugins", "lint@other"]),
                    remove_install(&installed, "lint@other", None),
                ],
                dirs(home, "lint", "other"),
                vec![
                    remove_key(&known, &["other"]),
                    remove_key(&global_settings, &["extraKnownMarketplaces", "other"]),
                ],
            ]
            .concat()
        );
    }

    #[test]
    fn plugin_dirs_go_once_the_last_selected_scope_drops_the_id() {
        let run = |scopes: &[Scope]| {
            let f = fixture();
            let data = f.env.home.join(".claude/plugins/data/tools-acme");
            fs::create_dir_all(&data).unwrap();
            let plan: Vec<Action> = scopes
                .iter()
                .flat_map(|&s| Claude.discover(&f.env, s).unwrap())
                .filter(|a| a.kind == Kind::Plugin && a.name == "tools@acme")
                .flat_map(|a| a.removal)
                .collect();
            crate::action::execute(&plan, &crate::runner::NoCommands).unwrap();
            let installed = read_json(&installed_file(&f.env.claude_dir())).unwrap();
            let kept = installed["plugins"].get("tools@acme").is_some();
            (kept, f.cache.exists(), data.exists())
        };
        assert_eq!(run(&[Scope::Global]), (true, true, true));
        assert_eq!(run(&[Scope::Project]), (true, true, true));
        assert_eq!(run(&[Scope::Global, Scope::Project]), (false, false, false));
    }

    #[test]
    fn install_record_edit_drops_one_scope() {
        let doc = json!({"version": 2, "plugins": {
            "p@m": [
                {"scope": "user", "installPath": "/c"},
                {"scope": "project", "projectPath": "/proj"},
                {"scope": "local", "projectPath": "/other"}
            ],
            "q@m": [{"scope": "user"}]
        }});
        let installs = installs_from(&doc);
        let after = |id: &str, project: Option<&str>| {
            let install = installs
                .iter()
                .find(|i| i.id == id && i.project.as_deref() == project.map(Path::new))
                .unwrap();
            let record_edit = plugin_removal(Path::new("/h/.claude"), install)
                .into_iter()
                .find(|a| matches!(a, Action::Edit { file, .. } if file.ends_with("installed_plugins.json")));
            let Some(Action::Edit { file, edit }) = record_edit else {
                panic!("no install record edit");
            };
            assert_eq!(file, installed_file(Path::new("/h/.claude")));
            let out = apply_edit(&file, &edit, &doc.to_string()).unwrap();
            serde_json::from_str::<Value>(&out).unwrap()["plugins"].clone()
        };
        assert_eq!(
            after("p@m", None)["p@m"],
            json!([
                {"scope": "project", "projectPath": "/proj"},
                {"scope": "local", "projectPath": "/other"}
            ])
        );
        assert_eq!(
            after("p@m", Some("/proj"))["p@m"],
            json!([
                {"scope": "user", "installPath": "/c"},
                {"scope": "local", "projectPath": "/other"}
            ])
        );
        let last = after("q@m", None);
        assert!(last.get("q@m").is_none());
        assert!(last.get("p@m").is_some());
    }

    #[test]
    fn plugin_dirs_need_both_halves_of_the_id() {
        // Segment rules are `RemovalTarget`'s; here the data dir must not outlive a bad cache dir.
        for id in ["bare", "@acme", "x@", "x@.."] {
            assert_eq!(plugin_dirs(Path::new("/h/.claude"), id), [], "{id}");
        }
        assert_eq!(plugin_dirs(Path::new("/h/.claude"), "a@b@acme").len(), 2);
    }

    #[test]
    fn marketplace_keeps_locations_outside_its_dir() {
        let home = Path::new("/h");
        for loc in [
            "/Users/x/Code/repo",
            "/h/.claude/plugins/marketplaces",
            "/h/.claude/plugins/marketplaces/../../..",
            "/h/.claude/plugins/marketplaces/a/../../x",
            "/h/.claude/plugins/marketplaces/a/b",
            "",
        ] {
            let m = Marketplace {
                name: "local".into(),
                source: Some("/Users/x/Code/repo".into()),
                install_location: loc.into(),
            };
            assert_eq!(
                files(&marketplace(&home.join(".claude"), &m, &[]).removal),
                [
                    remove_key(&marketplaces_file(&home.join(".claude")), &["local"]),
                    remove_key(
                        &home.join(".claude/settings.json"),
                        &["extraKnownMarketplaces", "local"]
                    ),
                ],
                "{loc}"
            );
        }
    }

    #[test]
    fn missing_files_are_empty() {
        let tmp = TempDir::new().unwrap();
        let env = Env {
            home: tmp.path().join("home"),
            project: tmp.path().join("proj"),
            claude_config: None,
            codex_home: None,
        };
        assert!(Claude.discover(&env, Scope::Global).unwrap().is_empty());
        assert!(Claude.discover(&env, Scope::Project).unwrap().is_empty());
    }

    #[test]
    fn malformed_json_is_err() {
        for file in [installed_file, marketplaces_file] {
            let f = fixture();
            write(&file(&f.env.claude_dir()), "{not json");
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

    #[test]
    fn usage_counts_loose_skills_and_used_plugins() {
        let json = json!({
            "skillUsage": {
                "tdd": {"usageCount": 3, "lastUsedAt": 10},
                "caveman:caveman": {"usageCount": 1, "lastUsedAt": 20},
                "broken": {"usageCount": 1}
            },
            "pluginUsage": {
                "caveman@caveman": {"usageCount": 5, "lastUsedAt": 30},
                "idle@mp": {"usageCount": 0, "lastUsedAt": 40}
            }
        });
        let mut uses = claude_uses(&json);
        uses.sort();
        assert_eq!(
            uses,
            [
                ("plugin:caveman@caveman".into(), 30),
                ("pskill:caveman:caveman".into(), 20),
                ("skill:tdd".into(), 10)
            ]
        );
    }

    #[test]
    fn plugins_and_marketplaces_remove_with_claude_first() {
        let home = Path::new("/h");
        let user = Install {
            id: "p@mp".into(),
            project: None,
            scope: "user".into(),
            install_path: None,
        };
        let local = Install {
            project: Some(PathBuf::from("/proj")),
            scope: "local".into(),
            ..user.clone()
        };
        let first = |i: &Install| {
            plugin_removal(&home.join(".claude"), i)
                .into_iter()
                .next()
                .unwrap()
        };
        let uninstall = |scope: &str, cwd: &str| Action::Run {
            program: "claude".into(),
            args: ["plugin", "uninstall", "p@mp", "--scope", scope]
                .map(String::from)
                .to_vec(),
            cwd: cwd.into(),
            fallback: true,
        };
        assert_eq!(first(&user), uninstall("user", "/h/.claude"));
        assert_eq!(first(&local), uninstall("local", "/proj"));
        let flag = Install {
            id: "-x@mp".into(),
            ..user.clone()
        };
        assert!(
            !plugin_removal(&home.join(".claude"), &flag)
                .iter()
                .any(|a| matches!(a, Action::Run { .. }))
        );

        let m = Marketplace {
            name: "mp".into(),
            source: None,
            install_location: String::new(),
        };
        let plan = marketplace(&home.join(".claude"), &m, &[user]).removal;
        let runs: Vec<&Action> = plan
            .iter()
            .filter(|a| matches!(a, Action::Run { .. }))
            .collect();
        assert_eq!(runs.len(), 2);
        assert_eq!(*runs[0], uninstall("user", "/h/.claude"));
        assert!(
            matches!(runs[1], Action::Run { args, .. } if args[..3] == ["plugin", "marketplace", "remove"])
        );
    }

    #[test]
    fn context_tokens_forms() {
        let t = context_tokens;
        assert_eq!(t("~100"), Some(100));
        assert_eq!(t("1.2k"), Some(1200));
        assert_eq!(t("~1.3k"), Some(1300));
        assert_eq!(t("< 20"), Some(10));
        assert_eq!(t("Tokens"), None);
        assert_eq!(t("------"), None);
    }

    #[test]
    fn context_report_gives_loose_skills_and_per_plugin_sums() {
        let text = include_str!("../../tests/fixtures/claude_context.md");
        let rows = parse_context(text);
        let sum = |key: &str| -> u32 { rows.iter().filter(|(k, _)| k == key).map(|r| r.1).sum() };
        assert_eq!(sum("skill:tdd"), 50);
        assert_eq!(sum("skill:dataviz"), 0, "built-in skills are not ours");
        // caveman: 21 skills plus 3 agents in the fixture
        assert!(sum("plugin:caveman") > 1500);
        assert!(
            rows.iter()
                .all(|(k, _)| k.starts_with("skill:") || k.starts_with("plugin:"))
        );
    }

    #[test]
    fn missing_claude_falls_back_to_estimates() {
        let tmp = tempfile::tempdir().unwrap();
        let skill = tmp.path().join("home/.claude/skills/tdd");
        std::fs::create_dir_all(&skill).unwrap();
        std::fs::write(skill.join("SKILL.md"), "---\nname: tdd\n---\n").unwrap();
        let env = Env {
            home: tmp.path().join("home"),
            project: tmp.path().join("proj"),
            claude_config: None,
            codex_home: None,
        };
        let rows = Claude
            .context_costs(&env, &crate::runner::NoCommands)
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "skill:tdd");
    }

    #[test]
    fn leftovers_match_plugin_named_files_but_not_a_longer_plugin_s() {
        let home = TempDir::new().unwrap();
        let claude = home.path().join(".claude");
        fs::create_dir_all(claude.join("plugins/store")).unwrap();
        for f in [
            ".killall-active",
            "killall-modes",
            "killall-agents-state",
            "killallx",
            "plugins/store/killall_mp-1.json",
        ] {
            fs::write(claude.join(f), "").unwrap();
        }
        fs::write(
            claude.join("plugins/installed_plugins.json"),
            json!({"plugins": {"killall@mp": [], "killall-agents@mp": []}}).to_string(),
        )
        .unwrap();
        let names: Vec<String> = leftovers(&home.path().join(".claude"), "killall@mp")
            .iter()
            .map(|a| format!("{a:?}"))
            .collect();
        let has = |f: &str| names.iter().any(|n| n.contains(f));
        assert_eq!(names.len(), 3);
        assert!(has(".killall-active") && has("killall-modes") && has("killall_mp-1.json"));
        assert!(leftovers(&home.path().join(".claude"), "ab@mp").is_empty());
    }

    #[test]
    fn claude_config_dir_replaces_dot_claude_for_global_only() {
        let f = fixture();
        let custom = f.env.home.join("isolated");
        let env = Env {
            claude_config: Some(custom.clone()),
            ..f.env.clone()
        };
        assert_eq!(
            Claude.skill_dirs(&env, Scope::Global),
            [custom.join("skills")]
        );
        assert_eq!(
            Claude.hook_files(&env, Scope::Global),
            [custom.join("settings.json")]
        );
        assert_eq!(
            Claude.skill_dirs(&env, Scope::Project),
            [env.project.join(".claude/skills")]
        );
        // Nothing is read from the default dir, so its plugins stay invisible.
        assert!(Claude.discover(&env, Scope::Global).unwrap().is_empty());
        assert_eq!(env.claude_json(), custom.join(".claude.json"));
        assert_eq!(f.env.claude_json(), f.env.home.join(".claude.json"));
    }
}
