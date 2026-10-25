mod context;
mod files;
mod marketplace;
mod mcp;
mod plugins;
mod records;
mod removal;
#[cfg(test)]
mod testutil;
pub(crate) mod update;
mod usage;

use std::path::{Path, PathBuf};

use super::{Harness, PluginOffer};
use crate::harness::Spec;
use crate::harness::action::Action;
use crate::harness::env::Env;
use crate::harness::hooks;
use crate::harness::model::{Agent, Installed, Kind, Scope};
use crate::harness::runner::CommandRunner;
use crate::harness::subagents::{self, AgentDir};
use context::parse_context;
use files::{
    agents_dir, hook_settings, installed_file, marketplaces_file, read_json, settings_files,
};
use records::{installs_from, marketplaces_from};

/// Claude's global config dir: `CLAUDE_CONFIG_DIR` when set, else `~/.claude`.
pub(crate) fn dir(env: &Env) -> PathBuf {
    env.claude_config
        .clone()
        .unwrap_or_else(|| env.home.join(".claude"))
}

/// Claude's global state file: inside `CLAUDE_CONFIG_DIR` when set, else `~/.claude.json`.
pub(crate) fn json(env: &Env) -> PathBuf {
    match &env.claude_config {
        Some(dir) => dir.join(".claude.json"),
        None => env.home.join(".claude.json"),
    }
}

pub struct Claude;

impl Harness for Claude {
    fn agent(&self) -> Agent {
        Agent::Claude
    }

    fn plan_update(&self, item: &Installed, cwd: &std::path::Path) -> Vec<Action> {
        update::steps(item, cwd)
    }

    fn home_dir_names(&self) -> &'static [&'static str] {
        &["claude"]
    }

    fn plugin_root_var(&self) -> Option<&'static str> {
        Some("CLAUDE_PLUGIN_ROOT")
    }

    fn hook_files(&self, env: &Env, scope: Scope) -> Vec<PathBuf> {
        settings_files(env, scope)
    }

    fn installed(&self, env: &Env, scope: Scope) -> anyhow::Result<Vec<Installed>> {
        let installs = installs_from(&read_json(&installed_file(&dir(env)))?);
        let markets = marketplaces_from(&read_json(&marketplaces_file(&dir(env)))?);
        let mut out = plugins::plugins(env, scope, &installs, &markets)?;
        out.extend(mcp::discover(env, scope)?);
        let agents = agents_dir(env, scope);
        out.extend(subagents::discover(
            &AgentDir::new(&agents, "md"),
            Agent::Claude,
            scope,
        )?);
        if scope == Scope::Global {
            out.extend(
                markets
                    .iter()
                    .map(|m| marketplace::discover(&dir(env), m, &installs)),
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
                    .flat_map(|&s| crate::harness::npx::skill_dirs(env, Agent::Claude, s))
                    .collect();
                Ok(crate::harness::cost::estimate_dirs(&dirs))
            }
        }
    }

    fn plan_install(&self, entry: &Spec, scope: Scope, env: &Env) -> Option<Vec<Action>> {
        match entry {
            Spec::Hook { .. } => hooks::provision(entry, hook_settings(env, scope)),
            Spec::Mcp { .. } => mcp::provision(entry, scope, env),
            Spec::Subagent { .. } => {
                subagents::provision(entry, &AgentDir::new(&agents_dir(env, scope), "md"), env)
            }
            Spec::Plugin { .. } => marketplace::provision(entry, scope, &env.project),
            Spec::Skill { .. } => None,
        }
    }

    fn plugin_offer(&self, src: &Path, _scope: Scope) -> anyhow::Result<Option<PluginOffer>> {
        marketplace::plugin_offer(src)
    }

    fn plan_add_source(
        &self,
        offer: &PluginOffer,
        picked: &[String],
        source: &str,
        scope: Scope,
        env: &Env,
    ) -> Vec<Action> {
        marketplace::plugin_steps(offer, picked, source, scope, &env.project)
    }

    /// `~/.claude.json` `skillUsage` (loose skills by name; `plugin:skill` keys are plugin
    /// skills, counted through `pluginUsage`) and `pluginUsage` (`name@mp`, only when
    /// `usageCount > 0`), both with `lastUsedAt` in ms.
    fn usage(&self, env: &Env) -> anyhow::Result<Vec<(String, u64)>> {
        Ok(usage::claude_uses(&read_json(&json(env))?))
    }

    fn tracks(&self, item: &Installed) -> bool {
        matches!(item.kind, Kind::Skill | Kind::Plugin)
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::*;
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn hook_files_per_scope() {
        let env = Env {
            home: "/h".into(),
            project: "/p".into(),
            claude_config: None,
            codex_home: None,
        };
        assert_eq!(Claude.agent(), Agent::Claude);
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
    fn missing_files_are_empty() {
        let tmp = TempDir::new().unwrap();
        let env = Env {
            home: tmp.path().join("home"),
            project: tmp.path().join("proj"),
            claude_config: None,
            codex_home: None,
        };

        let global = Claude.installed(&env, Scope::Global).unwrap();
        let project = Claude.installed(&env, Scope::Project).unwrap();

        assert!(global.is_empty());
        assert!(project.is_empty());
    }

    #[test]
    fn malformed_json_is_err() {
        for file in [installed_file, marketplaces_file] {
            let f = fixture();
            write(&file(&dir(&f.env)), "{not json");

            let global = Claude.installed(&f.env, Scope::Global);
            let project = Claude.installed(&f.env, Scope::Project);

            assert!(global.is_err());
            assert!(project.is_err());
        }
    }

    #[test]
    fn missing_claude_falls_back_to_estimates() {
        let tmp = tempfile::tempdir().unwrap();
        let skill = tmp.path().join("home/.claude/skills/tdd");
        fs::create_dir_all(&skill).unwrap();
        fs::write(skill.join("SKILL.md"), "---\nname: tdd\n---\n").unwrap();
        let env = Env {
            home: tmp.path().join("home"),
            project: tmp.path().join("proj"),
            claude_config: None,
            codex_home: None,
        };

        let rows = Claude
            .context_costs(&env, &crate::harness::runner::NoCommands)
            .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "skill:tdd");
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
            crate::harness::npx::skill_dirs(&env, Agent::Claude, Scope::Global),
            [custom.join("skills")]
        );
        assert_eq!(
            Claude.hook_files(&env, Scope::Global),
            [custom.join("settings.json")]
        );
        assert_eq!(
            crate::harness::npx::skill_dirs(&env, Agent::Claude, Scope::Project),
            [env.project.join(".claude/skills")]
        );
        // Nothing is read from the default dir, so its plugins stay invisible.
        assert!(Claude.installed(&env, Scope::Global).unwrap().is_empty());
        assert_eq!(json(&env), custom.join(".claude.json"));
        assert_eq!(json(&f.env), f.env.home.join(".claude.json"));
    }
}
