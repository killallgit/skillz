mod config;
mod mcp;
mod plugins;
mod removal;
mod timestamp;
mod usage;

#[cfg(test)]
mod testutil;

use std::path::PathBuf;

use super::AgentAdapter;
use crate::action::Action;
use crate::env::Env;
use crate::hooks;
use crate::model::{Agent, Augmentation, Kind, Scope};
use crate::npx_skills;
use crate::profile::Entry;
use crate::subagents::{self, AgentDir};
use config::{project_plugin_ids, read_config};
use plugins::{Cx, marketplaces, marketplaces_from, plugins_from};

pub struct Codex;

impl AgentAdapter for Codex {
    fn agent(&self) -> Agent {
        Agent::Codex
    }

    fn npx_id(&self) -> &'static str {
        "codex"
    }

    fn home_dir_names(&self) -> &'static [&'static str] {
        &["codex", "agents"]
    }

    fn skill_dirs(&self, env: &Env, scope: Scope) -> Vec<PathBuf> {
        match scope {
            Scope::Global => vec![
                env.codex_dir().join("skills"),
                npx_skills::store_dir(env, scope),
            ],
            Scope::Project => vec![
                npx_skills::store_dir(env, scope),
                env.project.join(".codex/skills"),
            ],
        }
    }

    fn hook_files(&self, env: &Env, scope: Scope) -> Vec<PathBuf> {
        let dir = env.codex_scope_dir(scope);
        vec![dir.join("hooks.json"), dir.join("config.toml")]
    }

    fn discover(&self, env: &Env, scope: Scope) -> anyhow::Result<Vec<Augmentation>> {
        let config = env.codex_scope_dir(scope).join("config.toml");
        let agents_dir = env.codex_scope_dir(scope).join("agents");
        let agents = subagents::discover(&AgentDir::new(&agents_dir, "toml"), Agent::Codex, scope)?;
        let Some(doc) = read_config(&config)? else {
            return Ok(agents);
        };
        let in_project = project_plugin_ids(env, &config);
        let cache = env.codex_dir().join("plugins/cache");
        let marketplaces = marketplaces(&doc);
        let cx = Cx {
            scope,
            config: &config,
            cache: &cache,
            marketplaces: &marketplaces,
            doc: &doc,
        };
        let plugins = plugins_from(&cx, in_project.as_deref());
        let mps = marketplaces_from(&cx, &plugins, in_project.as_deref());
        let servers = mcp::discover(&doc, scope, &config);
        Ok(mps
            .into_iter()
            .chain(plugins)
            .chain(servers)
            .chain(agents)
            .collect())
    }

    /// `codex plugin …` and `codex mcp …` only work on the user config, so project scope has no
    /// way in for those.
    fn provision(&self, entry: &Entry, scope: Scope, env: &Env) -> Option<Vec<Action>> {
        match entry {
            Entry::Hook { .. } => {
                hooks::provision(entry, env.codex_scope_dir(scope).join("hooks.json"))
            }
            Entry::Mcp { .. } => mcp::provision(entry, scope, env),
            Entry::Subagent { .. } => {
                let to = env.codex_scope_dir(scope).join("agents");
                subagents::provision(entry, &AgentDir::new(&to, "toml"), env)
            }
            Entry::Plugin { .. } => plugins::provision(entry, scope, env),
            Entry::Skill { .. } => None,
        }
    }

    /// Codex keeps no counters: a tool call in a session log under
    /// `$CODEX_HOME/{sessions,archived_sessions}` whose input reads `.../skills/<name>/SKILL.md`
    /// is a use, and a path under `plugins/cache/<mp>/<plugin>/` is a use of that plugin.
    fn usage(&self, env: &Env) -> anyhow::Result<Vec<(String, u64)>> {
        Ok(usage::all_uses(env))
    }

    /// Only skill reads leave a record, so a plugin without skills never shows a use.
    fn tracks(&self, item: &Augmentation) -> bool {
        item.kind == Kind::Skill
            || (item.kind == Kind::Plugin && item.plugin_skills().next().is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::*;
    use super::*;
    use crate::action::Action;
    use crate::model::Installer;
    use std::fs;

    #[test]
    fn paths_per_scope() {
        let env = Env {
            home: "/h".into(),
            project: "/p".into(),
            claude_config: None,
            codex_home: None,
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
        assert_eq!(review.installer, Installer::CodexCli);
        assert_eq!(review.name, "review@killall");
        assert_eq!(review.group, "killall");
        assert_eq!(review.agents, vec![Agent::Codex]);
        assert_eq!(review.scope, Scope::Global);
        assert_eq!(review.source.as_deref(), Some("killallgit/killall-skills"));
        assert_eq!(review.children.len(), 3);
        assert_eq!(
            files(&review.removal),
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
        assert_eq!(mp.installer, Installer::CodexCli);
        assert_eq!(mp.name, "killall");
        assert_eq!(mp.group, "killall");
        assert_eq!(mp.source.as_deref(), Some("killallgit/killall-skills"));
        assert_eq!(mp.children.len(), 2);
        assert_eq!(
            files(&mp.removal),
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
            files(&local.removal),
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
            files(&find(&items, "plugin:codex:global:review@killall").removal),
            review
        );
        assert_eq!(
            files(&find(&items, "plugin:codex:global:commit@killall").removal),
            commit
        );
        assert_eq!(
            files(&find(&items, "marketplace:codex:global:killall").removal),
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
                .all(|a| matches!(a, Action::Edit { .. } | Action::Run { .. }))
        );

        // Run from home: the "project" config is the global one and protects nothing.
        let home = Env {
            home: env.home.clone(),
            project: env.home.clone(),
            claude_config: None,
            codex_home: None,
        };
        let items = Codex.discover(&home, Scope::Global).unwrap();
        assert!(
            find(&items, "plugin:codex:global:review@killall")
                .removal
                .contains(&rm(&cache, &["killall", "review"]))
        );
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
            files(&find(&items, "marketplace:codex:global:..").removal),
            vec![
                rm_key(&config, "plugins", "x@.."),
                rm_key(&config, "marketplaces", ".."),
            ]
        );
        assert_eq!(
            files(&find(&items, "marketplace:codex:global:a/b").removal),
            vec![rm_key(&config, "marketplaces", "a/b")]
        );
        assert_eq!(
            files(&find(&items, r"plugin:codex:global:..\evil@ok").removal),
            vec![rm_key(&config, "plugins", r"..\evil@ok")]
        );
        assert_eq!(
            files(&find(&items, "plugin:codex:global:p@").removal),
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
            files(&review.removal),
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
        assert_eq!(
            files(&mp.removal),
            vec![rm_key(&config, "marketplaces", "team")]
        );
    }

    #[test]
    fn missing_config_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let env = Env {
            home: dir.path().into(),
            project: dir.path().into(),
            claude_config: None,
            codex_home: None,
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
    fn codex_home_replaces_dot_codex_for_global_only() {
        let env = Env {
            home: PathBuf::from("/h"),
            project: PathBuf::from("/p"),
            claude_config: None,
            codex_home: Some(PathBuf::from("/iso")),
        };
        let skills = Codex.skill_dirs(&env, Scope::Global);
        assert_eq!(skills[0], PathBuf::from("/iso/skills"));
        assert_eq!(
            Codex.hook_files(&env, Scope::Global),
            [
                PathBuf::from("/iso/hooks.json"),
                PathBuf::from("/iso/config.toml")
            ]
        );
        assert_eq!(
            Codex.hook_files(&env, Scope::Project)[0],
            PathBuf::from("/p/.codex/hooks.json")
        );
        assert_eq!(env.codex_dir(), PathBuf::from("/iso"));
    }
}
