//! Discovery of installed plugins as augmentations.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::records::{Install, Marketplace, mp_of, same_path};
use super::removal::plugin_removal;
use crate::env::Env;
use crate::model::{Agent, Augmentation, Installer, Kind, SKILL_CHILD, Scope};

/// One plugin augmentation per id among the installs belonging to `scope`.
pub(super) fn plugins(
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
        .map(|i| plugin(env, scope, i, markets))
        .collect()
}

fn plugin(env: &Env, scope: Scope, install: &Install, markets: &[Marketplace]) -> Augmentation {
    let mp = mp_of(&install.id);
    Augmentation {
        id: format!("plugin:claude:{scope}:{}", install.id),
        name: install.id.clone(),
        kind: Kind::Plugin,
        agents: vec![Agent::Claude],
        scope,
        group: mp.into(),
        source: markets
            .iter()
            .find(|m| m.name == mp)
            .and_then(|m| m.source.clone()),
        installer: Installer::ClaudeCli,
        children: install
            .install_path
            .as_deref()
            .map(plugin_children)
            .unwrap_or_default(),
        removal: plugin_removal(&env.claude_dir(), install),
    }
}

pub(super) fn dir_entries(dir: &Path) -> Vec<PathBuf> {
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

#[cfg(test)]
mod tests {
    use super::super::Claude;
    use super::super::files::installed_file;
    use super::super::testutil::*;
    use super::*;
    use crate::agents::AgentAdapter;

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
}
