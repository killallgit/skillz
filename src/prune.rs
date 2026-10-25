//! `skillz prune`: which installed items look redundant. Pure: callers pass the inventory,
//! the usage data and the current time.

use crate::agents::Harness;
use crate::model::{Agent, Installed, Kind, Scope};
use crate::usage::Usage;

/// Older than this counts as unused.
pub const UNUSED_AFTER_MS: u64 = 7 * 24 * 60 * 60 * 1000;

/// Each item appears in at most one list, checked in this order.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Findings<'a> {
    /// Loose skills every one of whose agents also gets the skill from an installed plugin.
    pub duplicates: Vec<&'a Installed>,
    pub never_used: Vec<&'a Installed>,
    /// With the last use in ms since the epoch.
    pub unused: Vec<(&'a Installed, u64)>,
}

pub fn check<'a>(
    items: &'a [Installed],
    adapters: &[Box<dyn Harness>],
    usage: &Usage,
    now_ms: u64,
) -> Findings<'a> {
    let mut found = Findings::default();
    for item in items {
        if is_duplicate(item, items) {
            found.duplicates.push(item);
        } else if tracked(item, adapters) {
            match usage.last_used(item) {
                None => found.never_used.push(item),
                Some(ms) if now_ms.saturating_sub(ms) > UNUSED_AFTER_MS => {
                    found.unused.push((item, ms))
                }
                Some(_) => {}
            }
        }
    }
    found
}

fn is_duplicate(skill: &Installed, items: &[Installed]) -> bool {
    // A global plugin serves every project; a project plugin only its own project's skills.
    let covers = |p: &Installed, agent: Agent| {
        p.kind == Kind::Plugin
            && p.agents.contains(&agent)
            && (p.scope == Scope::Global || p.scope == skill.scope)
            && p.plugin_skills().any(|s| s == skill.name)
    };
    skill.kind == Kind::Skill
        && skill
            .agents
            .iter()
            .all(|&a| items.iter().any(|p| covers(p, a)))
}

/// Items some adapter can see a use of (hooks and marketplaces never are).
pub fn tracked(item: &Installed, adapters: &[Box<dyn Harness>]) -> bool {
    matches!(item.kind, Kind::Skill | Kind::Plugin)
        && adapters
            .iter()
            .any(|a| item.agents.contains(&a.agent()) && a.tracks(item))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::registry;
    use crate::model::Details;
    use crate::model::Installer;
    use serde_json::json;

    fn item(kind: Kind, name: &str, agents: &[Agent], scope: Scope) -> Installed {
        Installed {
            details: Default::default(),
            hooks: Vec::new(),
            members: Vec::new(),
            id: format!("{kind}:{scope}:{name}"),
            name: name.into(),
            kind,
            agents: agents.to_vec(),
            scope,
            group: "g".into(),
            source: None,
            installer: Installer::Local,
            origin: Default::default(),
            children: vec![],
            removal: vec![],
        }
    }

    fn with_skills(item: Installed, skills: &[&str]) -> Installed {
        Installed {
            details: Details::Plugin {
                location: Default::default(),
                skills: skills.iter().map(|s| s.to_string()).collect(),
            },
            ..item
        }
    }

    fn skill(name: &str, agents: &[Agent], scope: Scope) -> Installed {
        item(Kind::Skill, name, agents, scope)
    }

    fn plugin(name: &str, agent: Agent, scope: Scope, skills: &[&str]) -> Installed {
        with_skills(item(Kind::Plugin, name, &[agent], scope), skills)
    }

    fn ids<'a>(items: impl IntoIterator<Item = &'a Installed>) -> Vec<&'a str> {
        items.into_iter().map(|i| i.id.as_str()).collect()
    }

    const DAY: u64 = 24 * 60 * 60 * 1000;
    const NOW: u64 = 100 * DAY;

    /// Usage where every Claude skill/plugin named in `used` was last used `days` ago.
    fn usage(used: &[(&str, u64)]) -> Usage {
        let home = tempfile::tempdir().unwrap();
        let mut skills = serde_json::Map::new();
        let mut plugins = serde_json::Map::new();
        for (key, days) in used {
            let entry = json!({"usageCount": 1, "lastUsedAt": NOW - days * DAY});
            match key.split_once(':') {
                Some(("plugin", id)) => plugins.insert(id.into(), entry),
                _ => skills.insert(key.to_string(), entry),
            };
        }
        let doc = json!({"skillUsage": skills, "pluginUsage": plugins});
        std::fs::write(home.path().join(".claude.json"), doc.to_string()).unwrap();
        let env = crate::env::Env {
            home: home.path().into(),
            project: home.path().into(),
            claude_config: None,
            codex_home: None,
        };
        crate::usage::read(&registry(), &env).unwrap()
    }

    #[test]
    fn loose_skill_is_a_duplicate_only_when_every_agent_has_it_from_a_plugin() {
        let items = vec![
            plugin("tools@mp", Agent::Claude, Scope::Global, &["tdd", "lint"]),
            plugin("proj@mp", Agent::Claude, Scope::Project, &["ship"]),
            skill("tdd", &[Agent::Claude], Scope::Project),
            skill("lint", &[Agent::Claude, Agent::Codex], Scope::Global),
            skill("ship", &[Agent::Claude], Scope::Global),
            skill("ship", &[Agent::Claude], Scope::Project),
            skill("other", &[Agent::Claude], Scope::Global),
        ];
        let found = check(&items, &registry(), &usage(&[]), NOW);
        assert_eq!(
            ids(found.duplicates),
            ["skill:project:tdd", "skill:project:ship"]
        );
    }

    #[test]
    fn unused_splits_never_from_older_than_seven_days() {
        let items = vec![
            skill("fresh", &[Agent::Claude], Scope::Global),
            skill("old", &[Agent::Claude], Scope::Global),
            skill("never", &[Agent::Claude], Scope::Global),
            plugin("idle@mp", Agent::Claude, Scope::Global, &[]),
            plugin("busy@mp", Agent::Claude, Scope::Global, &[]),
        ];
        let used = usage(&[("fresh", 6), ("old", 8), ("plugin:busy@mp", 0)]);
        let found = check(&items, &registry(), &used, NOW);
        assert_eq!(
            ids(found.never_used),
            ["skill:global:never", "plugin:global:idle@mp"]
        );
        assert_eq!(found.unused.len(), 1);
        assert_eq!(found.unused[0].0.id, "skill:global:old");
        assert_eq!(found.unused[0].1, NOW - 8 * DAY);
        assert!(found.duplicates.is_empty());
    }

    #[test]
    fn items_without_a_usage_source_are_never_flagged() {
        let mut hook = skill("rtk", &[Agent::Claude], Scope::Global);
        hook.kind = Kind::Hook;
        let items = vec![
            hook,
            plugin("hooks-only@mp", Agent::Codex, Scope::Global, &[]),
            item(Kind::Marketplace, "mp", &[Agent::Claude], Scope::Global),
        ];
        assert_eq!(
            check(&items, &registry(), &usage(&[]), NOW),
            Findings::default()
        );
        let codex_skills = [plugin("p@mp", Agent::Codex, Scope::Global, &["a"])];
        assert_eq!(
            check(&codex_skills, &registry(), &usage(&[]), NOW)
                .never_used
                .len(),
            1
        );
    }
}
