//! One adapter per agent. Adding an agent = a new module implementing `AgentAdapter` plus a
//! line in `registry()`.

pub mod claude;
pub mod codex;
pub mod cursor;
mod inventory;

pub use inventory::{Inventory, inventory};

use std::path::{Path, PathBuf};

use crate::action::Action;
use crate::env::Env;
use crate::model::{Agent, Augmentation, Scope};
use crate::profile::Entry;
use crate::prompt::Prompter;
use crate::runner::CommandRunner;

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
    /// Steps that install a profile `entry` for this agent at `scope`, or `None` when this agent
    /// has no way to take that entry there. Skills are not asked of adapters: `provision::plan`
    /// sends them through `npx skills` for every agent at once.
    fn provision(&self, _entry: &Entry, _scope: Scope, _env: &Env) -> Option<Vec<Action>> {
        None
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

/// The adapter among `adapters` that handles `agent`.
pub fn adapter_for(adapters: &[Box<dyn AgentAdapter>], agent: Agent) -> Option<&dyn AgentAdapter> {
    adapters
        .iter()
        .find(|a| a.agent() == agent)
        .map(|a| a.as_ref())
}

/// `agent`'s id for the `skills` CLI.
pub fn npx_id(agent: Agent) -> &'static str {
    adapter(agent).map_or("", |a| a.npx_id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_agent_is_registered_with_a_distinct_npx_id() {
        let mut ids: Vec<&str> = all().into_iter().map(npx_id).collect();
        assert!(ids.iter().all(|id| !id.is_empty()));
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), all().len());
    }
}
