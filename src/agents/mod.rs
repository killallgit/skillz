//! One adapter per agent. Adding an agent = a new module implementing `AgentAdapter` plus a
//! line in `registry()`.

pub(crate) mod claude;
pub(crate) mod codex;
mod inventory;
mod plugin;

pub use inventory::{Inventory, inventory};

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::action::Action;
use crate::env::Env;
use crate::model::{Agent, Installed, Scope};
use crate::profile::Spec;
use crate::runner::CommandRunner;

/// The plugins a fetched source offers one agent through its own installer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginOffer {
    pub marketplace: String,
    pub plugins: Vec<String>,
}

pub trait AgentAdapter {
    fn agent(&self) -> Agent;
    /// Directories this agent loads loose skills from, for `scope`.
    fn skill_dirs(&self, env: &Env, scope: Scope) -> Vec<PathBuf>;
    /// Hook config files this agent reads for `scope` (parsed by `hooks::discover`).
    fn hook_files(&self, env: &Env, scope: Scope) -> Vec<PathBuf>;
    /// Plugins and marketplaces for `scope`. Loose skills and hooks are discovered by the
    /// shared `skills`/`hooks` modules from `skill_dirs`/`hook_files`.
    fn discover(&self, env: &Env, scope: Scope) -> anyhow::Result<Vec<Installed>>;
    /// Hidden home dir names (`.claude` -> `claude`) this agent owns; hook grouping skips them.
    fn home_dir_names(&self) -> &'static [&'static str];
    /// Env var a plugin's hook commands use for the plugin's own dir, if any.
    fn plugin_root_var(&self) -> Option<&'static str> {
        None
    }
    /// The plugins the tree `src` offers this agent at `scope`, or `None` when it has no plugin
    /// install for the source; its skills then go through `npx skills`.
    fn plugin_offer(&self, _src: &Path, _scope: Scope) -> anyhow::Result<Option<PluginOffer>> {
        Ok(None)
    }
    /// Steps that add `source`'s marketplace and install the `picked` plugins of `offer`.
    fn plugin_steps(
        &self,
        _offer: &PluginOffer,
        _picked: &[String],
        _source: &str,
        _scope: Scope,
        _env: &Env,
    ) -> Vec<Action> {
        vec![]
    }
    /// Steps that install a profile `entry` for this agent at `scope`, or `None` when this agent
    /// has no way to take that entry there. Skills are not asked of adapters: `provision::plan`
    /// sends them through `npx skills` for every agent at once.
    fn provision(&self, _entry: &Spec, _scope: Scope, _env: &Env) -> Option<Vec<Action>> {
        None
    }
    fn update_steps(&self, _item: &Installed, _cwd: &Path) -> Vec<Action> {
        vec![]
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
    fn tracks(&self, _item: &Installed) -> bool {
        false
    }
}

pub fn registry() -> Vec<Box<dyn AgentAdapter>> {
    vec![Box::new(claude::Claude), Box::new(codex::Codex)]
}

/// The adapter among `adapters` that handles `agent`.
pub fn adapter_for(adapters: &[Box<dyn AgentAdapter>], agent: Agent) -> Option<&dyn AgentAdapter> {
    adapters
        .iter()
        .find(|a| a.agent() == agent)
        .map(|a| a.as_ref())
}

pub fn update_checker<R: CommandRunner + Send + Sync + 'static>(
    env: &Env,
    runner: R,
) -> Arc<dyn crate::update::check::UpdateCheck> {
    Arc::new(claude::update::ClaudeMarketplaces {
        runner,
        clones: env.claude_dir().join("plugins/marketplaces"),
    })
}
