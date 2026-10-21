//! One adapter per agent. Adding an agent = a new module implementing `AgentAdapter` plus a
//! line in `registry()`.

pub mod claude;
pub mod codex;
pub mod cursor;

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use crate::env::Env;
use crate::hooks;
use crate::model::{Agent, Augmentation, Kind, Scope};
use crate::prompt::Prompter;
use crate::runner::CommandRunner;
use crate::skills::{self, SkillDir};

/// What an adapter needs to install a source's plugins.
pub struct PluginInstall<'a> {
    /// The fetched tree to inspect.
    pub src: &'a Path,
    /// What to hand to the agent's own CLI (never a tmp clone).
    pub source: &'a str,
    pub scope: Scope,
    pub env: &'a Env,
    pub runner: &'a dyn CommandRunner,
    pub prompter: &'a dyn Prompter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginOutcome {
    /// The agent has no plugin install for this source; its skills go through `npx skills`.
    Unsupported,
    /// The user picked no plugins.
    Declined,
    Installed,
}

pub trait AgentAdapter {
    fn agent(&self) -> Agent;
    /// The id the `skills` CLI (`npx skills add -a <id>`) uses for this agent.
    fn npx_id(&self) -> &'static str;
    /// Directories this agent loads loose skills from, for `scope`.
    fn skill_dirs(&self, env: &Env, scope: Scope) -> Vec<PathBuf>;
    /// Hook config files this agent reads for `scope` (parsed by `hooks::discover`).
    fn hook_files(&self, env: &Env, scope: Scope) -> Vec<PathBuf>;
    /// Plugins and marketplaces for `scope`. Loose skills and hooks are discovered by the
    /// shared `skills`/`hooks` modules from `skill_dirs`/`hook_files`.
    fn discover(&self, env: &Env, scope: Scope) -> anyhow::Result<Vec<Augmentation>>;
    /// Hidden home dir names (`.claude` -> `claude`) this agent owns; hook grouping skips them.
    fn home_dir_names(&self) -> &'static [&'static str];
    /// Env var a plugin's hook commands use for the plugin's own dir, if any.
    fn plugin_root_var(&self) -> Option<&'static str> {
        None
    }
    /// Install the source's marketplace plugins through the agent's own CLI. The default is
    /// `Unsupported`.
    fn install_plugins(&self, _req: &PluginInstall) -> anyhow::Result<PluginOutcome> {
        Ok(PluginOutcome::Unsupported)
    }
    /// Last-use records `(key, ms since the epoch)`, keyed `skill:<name>` or
    /// `plugin:<name@mp>`. The default is no usage source.
    fn usage(&self, _env: &Env) -> anyhow::Result<Vec<(String, u64)>> {
        Ok(vec![])
    }
    /// Always-on context cost rows `(key, tokens)`, keyed `skill:<name>` or `plugin:<name>`
    /// (no `@marketplace`). The default estimates loose skills from their frontmatter.
    fn context_costs(
        &self,
        env: &Env,
        _runner: &dyn CommandRunner,
    ) -> anyhow::Result<Vec<(String, u32)>> {
        let dirs: Vec<PathBuf> = [Scope::Global, Scope::Project]
            .iter()
            .flat_map(|&s| self.skill_dirs(env, s))
            .collect();
        Ok(crate::cost::estimate_dirs(&dirs))
    }
    /// Whether `usage` can see `item`'s use. Items nobody tracks are never flagged as unused.
    fn tracks(&self, _item: &Augmentation) -> bool {
        false
    }
}

pub fn registry() -> Vec<Box<dyn AgentAdapter>> {
    vec![
        Box::new(claude::Claude),
        Box::new(codex::Codex),
        Box::new(cursor::Cursor),
    ]
}

/// Every registered agent, in registry order.
pub fn all() -> Vec<Agent> {
    registry().iter().map(|a| a.agent()).collect()
}

/// Hidden home dir names owned by any agent.
pub fn home_dir_names() -> Vec<&'static str> {
    registry()
        .iter()
        .flat_map(|a| a.home_dir_names())
        .copied()
        .collect()
}

/// `${VAR}` tokens that mark a command as living in a plugin's own dir.
pub fn plugin_root_vars() -> Vec<&'static str> {
    registry()
        .iter()
        .filter_map(|a| a.plugin_root_var())
        .collect()
}

pub fn adapter(agent: Agent) -> Option<Box<dyn AgentAdapter>> {
    registry().into_iter().find(|a| a.agent() == agent)
}

/// `agent`'s id for the `skills` CLI.
pub fn npx_id(agent: Agent) -> &'static str {
    adapter(agent).map_or("", |a| a.npx_id())
}

/// Everything installed, plus warnings for sources that failed to read.
#[derive(Debug, Default)]
pub struct Inventory {
    pub items: Vec<Augmentation>,
    pub warnings: Vec<String>,
}

/// Combine every adapter's discovery and the shared skill discovery for `scopes`.
/// Items are deduplicated by id; a failing source becomes a warning, not an error.
pub fn inventory(adapters: &[Box<dyn AgentAdapter>], env: &Env, scopes: &[Scope]) -> Inventory {
    let mut inv = Inventory::default();
    // One discover over every scope, so a project link into a global skill is its own item.
    let dirs: Vec<SkillDir> = scopes
        .iter()
        .flat_map(|&scope| {
            adapters.iter().flat_map(move |a| {
                a.skill_dirs(env, scope)
                    .into_iter()
                    .map(move |path| SkillDir {
                        agent: a.agent(),
                        scope,
                        path,
                    })
            })
        })
        .collect();
    inv.absorb("skills", skills::discover(env, &dirs));
    for &scope in scopes {
        for a in adapters {
            for file in a.hook_files(env, scope) {
                inv.absorb(
                    &file.display().to_string(),
                    hooks::discover(&file, a.agent(), scope),
                );
            }
            inv.absorb(&a.agent().to_string(), a.discover(env, scope));
        }
    }
    let mut seen = HashSet::new();
    inv.items.retain(|item| seen.insert(item.id.clone()));
    join_marketplace_groups(&mut inv.items);
    inv
}

/// npx skills group by their source repo; when a marketplace comes from that same repo, show
/// them under the marketplace instead of as a second group.
fn join_marketplace_groups(items: &mut [Augmentation]) {
    let by_source: BTreeMap<String, String> = items
        .iter()
        .filter(|i| i.kind == Kind::Marketplace)
        .filter_map(|i| Some((i.source.clone()?, i.group.clone())))
        .collect();
    for item in items.iter_mut() {
        if let Some(group) = by_source.get(&item.group) {
            item.group = group.clone();
        }
    }
}

impl Inventory {
    fn absorb(&mut self, source: &str, found: anyhow::Result<Vec<Augmentation>>) {
        match found {
            Ok(items) => self.items.extend(items),
            Err(err) => self.warnings.push(format!("{source}: {err:#}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Installer;

    fn item(name: &str, kind: Kind, group: &str, source: Option<&str>) -> Augmentation {
        Augmentation {
            id: format!("{kind}:{name}"),
            name: name.into(),
            kind,
            agents: vec![Agent::Codex],
            scope: Scope::Global,
            group: group.into(),
            source: source.map(Into::into),
            installer: Installer::Local,
            children: vec![],
            removal: vec![],
        }
    }

    #[test]
    fn every_agent_is_registered_with_a_distinct_npx_id() {
        let mut ids: Vec<&str> = all().into_iter().map(npx_id).collect();
        assert!(ids.iter().all(|id| !id.is_empty()));
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), all().len());
    }

    #[test]
    fn items_from_a_marketplace_repo_join_its_group() {
        let mut items = vec![
            item("mp", Kind::Marketplace, "mp", Some("o/r")),
            item("tdd", Kind::Skill, "o/r", Some("o/r")),
            item("other", Kind::Skill, "x/y", Some("x/y")),
            item("loose", Kind::Skill, "local", None),
        ];
        join_marketplace_groups(&mut items);
        let groups: Vec<&str> = items.iter().map(|i| i.group.as_str()).collect();
        assert_eq!(groups, ["mp", "mp", "x/y", "local"]);
    }
}
