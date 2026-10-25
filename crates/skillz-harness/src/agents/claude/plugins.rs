//! Discovery of installed plugins as installed items.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::records::{Install, Marketplace, same_path};
use super::removal::plugin_removal;
use crate::env::Env;
use crate::hooks;
use crate::model::{
    Agent, HOOK_CHILD, Installed, Installer, Kind, Origin, SKILL_CHILD, Scope, split_plugin_id,
};

/// One plugin item per id among the installs belonging to `scope`.
pub(super) fn plugins(
    env: &Env,
    scope: Scope,
    installs: &[Install],
    markets: &[Marketplace],
) -> anyhow::Result<Vec<Installed>> {
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
        .map(|i| plugin(env, scope, i, markets))
        .collect()
}

fn plugin(
    env: &Env,
    scope: Scope,
    install: &Install,
    markets: &[Marketplace],
) -> anyhow::Result<Installed> {
    let (_, mp) = split_plugin_id(&install.id);
    let source = markets
        .iter()
        .find(|m| m.name == mp)
        .and_then(|m| m.source.clone());
    let root = install.install_path.as_deref();
    let hooks = root
        .map(|r| hooks::plugin_hooks(r, &r.join(".claude-plugin/plugin.json")))
        .unwrap_or_default();
    let skills = root
        .map(super::super::plugin::skill_names)
        .transpose()?
        .unwrap_or_default();
    let mut children = skills
        .iter()
        .map(|name| format!("{SKILL_CHILD}{name}"))
        .collect::<Vec<_>>();
    if let Some(root) = root {
        children.extend(plugin_children(root)?);
    }
    children.extend(hooks.iter().map(|h| format!("{HOOK_CHILD}{}", h.label())));
    children.sort();
    Ok(Installed {
        details: crate::model::Details::Plugin {
            location: root
                .map(Path::to_path_buf)
                .unwrap_or_else(|| super::files::installed_file(&super::dir(env))),
            skills,
        },
        hooks,
        members: Vec::new(),
        id: format!("plugin:claude:{scope}:{}", install.id),
        name: install.id.clone(),
        kind: Kind::Plugin,
        agents: vec![Agent::Claude],
        scope,
        group: mp.into(),
        source: source.clone(),
        installer: Installer::ClaudeCli,
        origin: Origin {
            link: source,
            version: install.version.clone(),
        },
        children,
        removal: plugin_removal(&super::dir(env), install),
    })
}

pub(super) fn dir_entries(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.path()).collect())
        .unwrap_or_default()
}

fn plugin_children(root: &Path) -> anyhow::Result<Vec<String>> {
    let mut out = Vec::new();
    for (dir, label) in [("agents", "agent"), ("commands", "command")] {
        for entry in crate::read_optional::read_dir(&root.join(dir))? {
            let path = entry.path();
            if entry.file_type()?.is_file()
                && path.extension().is_some_and(|e| e == "md")
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            {
                out.push(format!("{label}: {stem}"));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::super::Claude;
    use super::super::files::installed_file;
    use super::super::testutil::*;
    use super::*;
    use crate::agents::Harness;

    #[test]
    fn global_plugins() {
        let f = fixture();
        let items = Claude.installed(&f.env, Scope::Global).unwrap();
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
        assert_eq!(tools.hooks.len(), 1);
        assert_eq!(tools.plugin_skills().collect::<Vec<_>>(), ["alpha", "beta"]);
        assert_eq!(tools.hooks[0].command, "say done");
        let installed = installed_file(&crate::agents::claude::dir(&f.env));
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
        let items = Claude.installed(&f.env, Scope::Project).unwrap();
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
                    remove_install(
                        &installed_file(&crate::agents::claude::dir(&f.env)),
                        "tools@acme",
                        Some(p)
                    ),
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

        let items = Claude.installed(&env, Scope::Project).unwrap();

        assert_eq!(
            items.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(),
            ["plugin:claude:project:tools@acme"]
        );
    }
}
