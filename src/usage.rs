//! When each skill and plugin was last used, from the agents' own records.
//!
//! Each adapter reports its own records (`AgentAdapter::usage`); this module only merges them.
//! Keys: `skill:<name>` (loose skill), `plugin:<name@mp>` (whole plugin: its hooks and other
//! parts too) and `pskill:<plugin>:<skill>` (one skill of a plugin).

use std::collections::HashMap;

use crate::agents::AgentAdapter;
use crate::env::Env;
use crate::model::{Agent, Augmentation, Kind};

/// Last use in ms since the epoch, per agent and `skill:<name>` / `plugin:<name@mp>` key.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Usage(HashMap<(Agent, String), u64>);

impl Usage {
    fn record(&mut self, agent: Agent, key: String, ms: u64) {
        let last = self.0.entry((agent, key)).or_default();
        *last = (*last).max(ms);
    }

    /// Last use of `skill` from the plugin `plugin`, by any of the plugin's agents. This is
    /// the skill alone: `last_used` on the plugin also counts its hooks and other parts.
    pub fn skill_last_used(&self, plugin: &Augmentation, skill: &str) -> Option<u64> {
        let name = plugin.name.split('@').next()?;
        let key = format!("pskill:{name}:{skill}");
        plugin
            .agents
            .iter()
            .filter_map(|&a| self.0.get(&(a, key.clone())))
            .max()
            .copied()
    }

    /// Latest use by any of the item's agents; `None` = never used (or not tracked: hooks,
    /// marketplaces).
    pub fn last_used(&self, item: &Augmentation) -> Option<u64> {
        let key = match item.kind {
            Kind::Skill => format!("skill:{}", item.name),
            Kind::Plugin => format!("plugin:{}", item.name),
            Kind::Marketplace | Kind::Hook => return None,
        };
        item.agents
            .iter()
            .filter_map(|&a| self.0.get(&(a, key.clone())))
            .max()
            .copied()
    }
}

/// Every adapter's usage records. An adapter whose source is unreadable fails the read.
pub fn read(adapters: &[Box<dyn AgentAdapter>], env: &Env) -> anyhow::Result<Usage> {
    let mut usage = Usage::default();
    for a in adapters {
        for (key, ms) in a.usage(env)? {
            usage.record(a.agent(), key, ms);
        }
    }
    Ok(usage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::registry;
    use crate::model::Installer;
    use crate::model::{Agent, Kind, Scope};
    use serde_json::json;
    use std::fs;

    fn call(kind: &str, field: &str, text: &str) -> String {
        json!({"type": "response_item", "payload": {"type": kind, field: text}}).to_string()
    }

    fn item(kind: Kind, name: &str, agents: &[Agent]) -> Augmentation {
        Augmentation {
            id: name.into(),
            name: name.into(),
            kind,
            agents: agents.to_vec(),
            scope: Scope::Global,
            group: "g".into(),
            source: None,
            installer: Installer::Local,
            children: vec![],
            removal: vec![],
        }
    }

    #[test]
    fn read_merges_agents_and_keeps_the_latest_use() {
        let home = tempfile::tempdir().unwrap();
        let env = Env {
            home: home.path().into(),
            project: home.path().join("p"),
        };
        fs::write(
            home.path().join(".claude.json"),
            json!({"skillUsage": {"tdd": {"lastUsedAt": 5}}}).to_string(),
        )
        .unwrap();
        let day = home.path().join(".codex/archived_sessions/2026/10");
        fs::create_dir_all(&day).unwrap();
        let lines = [
            call("custom_tool_call", "input", "cat /x/skills/tdd/SKILL.md"),
            call("custom_tool_call", "input", "cat /x/skills/tdd/SKILL.md"),
        ];
        fs::write(day.join("rollout-1.jsonl"), lines.join("\n")).unwrap();

        let usage = read(&registry(), &env).unwrap();
        let tdd = |agents: &[Agent]| usage.last_used(&item(Kind::Skill, "tdd", agents));
        assert_eq!(tdd(&[Agent::Claude]), Some(5));
        assert!(tdd(&[Agent::Codex]) > Some(5));
        assert_eq!(tdd(&[Agent::Claude, Agent::Codex]), tdd(&[Agent::Codex]));
        assert_eq!(tdd(&[Agent::Cursor]), None);
        assert_eq!(
            usage.last_used(&item(Kind::Hook, "tdd", &[Agent::Claude])),
            None
        );
    }

    #[test]
    fn missing_sources_are_empty_and_bad_claude_json_errors() {
        let home = tempfile::tempdir().unwrap();
        let env = Env {
            home: home.path().into(),
            project: home.path().into(),
        };
        assert_eq!(read(&registry(), &env).unwrap(), Usage::default());
        fs::write(home.path().join(".claude.json"), "{").unwrap();
        assert!(read(&registry(), &env).is_err());
    }

    #[test]
    fn plugin_skill_use_is_separate_from_plugin_use() {
        let home = tempfile::tempdir().unwrap();
        let env = Env {
            home: home.path().into(),
            project: home.path().into(),
        };
        fs::write(
            home.path().join(".claude.json"),
            json!({
                "skillUsage": {"caveman:caveman": {"lastUsedAt": 7}},
                "pluginUsage": {"caveman@caveman": {"usageCount": 9, "lastUsedAt": 9}}
            })
            .to_string(),
        )
        .unwrap();
        let usage = read(&registry(), &env).unwrap();
        let plugin = item(Kind::Plugin, "caveman@caveman", &[Agent::Claude]);
        assert_eq!(usage.skill_last_used(&plugin, "caveman"), Some(7));
        assert_eq!(usage.skill_last_used(&plugin, "caveman-help"), None);
        assert_eq!(usage.last_used(&plugin), Some(9));
    }
}
