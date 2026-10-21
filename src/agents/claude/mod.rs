mod context;
mod files;
mod marketplace;
mod mcp;
mod plugins;
mod records;
mod removal;
#[cfg(test)]
mod testutil;
mod usage;

use std::path::PathBuf;

use super::{AgentAdapter, PluginInstall, PluginOutcome};
use crate::action::Action;
use crate::env::Env;
use crate::hooks;
use crate::model::{Agent, Augmentation, Kind, Scope};
use crate::profile::Entry;
use crate::runner::CommandRunner;
use crate::subagents::{self, AgentDir};
use context::parse_context;
use files::{
    agents_dir, hook_settings, installed_file, marketplaces_file, read_json, settings_files,
};
use records::{installs_from, marketplaces_from};

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
        let mut out = plugins::plugins(env, scope, &installs, &markets);
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
                    .map(|m| marketplace::discover(&env.claude_dir(), m, &installs)),
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

    fn provision(&self, entry: &Entry, scope: Scope, env: &Env) -> Option<Vec<Action>> {
        match entry {
            Entry::Hook { .. } => hooks::provision(entry, hook_settings(env, scope)),
            Entry::Mcp { .. } => mcp::provision(entry, scope, env),
            Entry::Subagent { .. } => {
                subagents::provision(entry, &AgentDir::new(&agents_dir(env, scope), "md"), env)
            }
            Entry::Plugin {
                source,
                marketplace,
                name,
                ..
            } => Some(marketplace::provision(
                source,
                marketplace,
                name,
                scope,
                &env.project,
            )),
            Entry::Skill { .. } => None,
        }
    }

    fn install_plugins(&self, req: &PluginInstall) -> anyhow::Result<PluginOutcome> {
        marketplace::install_plugins(req)
    }

    /// `~/.claude.json` `skillUsage` (loose skills by name; `plugin:skill` keys are plugin
    /// skills, counted through `pluginUsage`) and `pluginUsage` (`name@mp`, only when
    /// `usageCount > 0`), both with `lastUsedAt` in ms.
    fn usage(&self, env: &Env) -> anyhow::Result<Vec<(String, u64)>> {
        Ok(usage::claude_uses(&read_json(&env.claude_json())?))
    }

    fn tracks(&self, item: &Augmentation) -> bool {
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
    fn missing_files_are_empty() {
        let tmp = TempDir::new().unwrap();
        let env = Env {
            home: tmp.path().join("home"),
            project: tmp.path().join("proj"),
            claude_config: None,
            codex_home: None,
        };

        let global = Claude.discover(&env, Scope::Global).unwrap();
        let project = Claude.discover(&env, Scope::Project).unwrap();

        assert!(global.is_empty());
        assert!(project.is_empty());
    }

    #[test]
    fn malformed_json_is_err() {
        for file in [installed_file, marketplaces_file] {
            let f = fixture();
            write(&file(&f.env.claude_dir()), "{not json");

            let global = Claude.discover(&f.env, Scope::Global);
            let project = Claude.discover(&f.env, Scope::Project);

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
            .context_costs(&env, &crate::runner::NoCommands)
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
