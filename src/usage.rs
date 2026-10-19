//! When each skill and plugin was last used, from the agents' own records.
//!
//! - Claude: `~/.claude.json` `skillUsage` (loose skills by name; `plugin:skill` keys are
//!   plugin skills, counted through `pluginUsage`) and `pluginUsage` (`name@mp`, only when
//!   `usageCount > 0`), both with `lastUsedAt` in ms.
//! - Codex keeps no counters: a tool call whose input reads `.../skills/<name>/SKILL.md` in a
//!   session log is a use; a path under `plugins/cache/<mp>/<plugin>/` is a use of that plugin.
//! - Cursor: no source.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, ErrorKind};
use std::path::Path;
use std::time::UNIX_EPOCH;

use anyhow::Context;
use serde_json::Value;

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

/// Claude's counters and every Codex session log under `~/.codex/{sessions,archived_sessions}`.
/// Unreadable session logs are skipped; a malformed `~/.claude.json` is an error.
pub fn read(env: &Env) -> anyhow::Result<Usage> {
    let mut usage = Usage::default();
    let claude_json = env.home.join(".claude.json");
    match fs::read_to_string(&claude_json) {
        Ok(text) => {
            let json: Value = serde_json::from_str(&text)
                .with_context(|| format!("parsing {}", claude_json.display()))?;
            for (key, ms) in claude_uses(&json) {
                usage.record(Agent::Claude, key, ms);
            }
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(e).with_context(|| format!("reading {}", claude_json.display())),
    }
    for dir in ["sessions", "archived_sessions"] {
        let pattern = glob::Pattern::escape(&env.home.join(".codex").join(dir).to_string_lossy());
        for log in glob::glob(&format!("{pattern}/**/*.jsonl"))
            .into_iter()
            .flatten()
            .flatten()
        {
            read_codex_log(&log, &mut usage);
        }
    }
    Ok(usage)
}

// shortcut: a use is dated by its log's mtime (the session's last write), not the record's
// timestamp; fine at the doctor's 7-day granularity.
fn read_codex_log(log: &Path, usage: &mut Usage) {
    let Ok(file) = fs::File::open(log) else {
        return;
    };
    let ms = file
        .metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as u64);
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if line.contains("SKILL.md") {
            for key in codex_uses(&line) {
                usage.record(Agent::Codex, key, ms);
            }
        }
    }
}

fn claude_uses(json: &Value) -> Vec<(String, u64)> {
    let entries = |field: &str| {
        json.get(field)
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter_map(|(k, v)| Some((k.clone(), v.get("lastUsedAt")?.as_u64()?, v)))
    };
    let skills = entries("skillUsage")
        .filter(|(k, _, _)| !k.contains(':'))
        .map(|(k, ms, _)| (format!("skill:{k}"), ms));
    let plugins = entries("pluginUsage")
        .filter(|(_, _, v)| v.get("usageCount").and_then(Value::as_u64) > Some(0))
        .map(|(k, ms, _)| (format!("plugin:{k}"), ms));
    skills.chain(plugins).collect()
}

/// Usage keys for one session log line: only tool calls count, since every session's prompt
/// lists the path of each installed skill.
fn codex_uses(line: &str) -> Vec<String> {
    let Ok(record) = serde_json::from_str::<Value>(line) else {
        return vec![];
    };
    let payload = &record["payload"];
    let is_call = record["type"] == "response_item"
        && matches!(
            payload["type"].as_str(),
            Some("custom_tool_call" | "function_call")
        );
    let input = payload["input"].as_str().or(payload["arguments"].as_str());
    match (is_call, input) {
        (true, Some(input)) => skill_paths(input).filter_map(use_key).collect(),
        _ => vec![],
    }
}

/// Every `.../SKILL.md` path in shell- or JS-ish text.
fn skill_paths(text: &str) -> impl Iterator<Item = &str> {
    text.match_indices("/SKILL.md").map(move |(end, m)| {
        let start = text[..end]
            .rfind(|c: char| c.is_whitespace() || "\"'`(=,:".contains(c))
            .map_or(0, |i| i + 1);
        &text[start..end + m.len()]
    })
}

fn use_key(path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split('/').collect();
    let n = parts.len();
    if n < 3 || parts[n - 3] != "skills" {
        return None;
    }
    let plugin = parts
        .windows(4)
        .find(|w| w[0] == "plugins" && w[1] == "cache")
        .map(|w| format!("plugin:{}@{}", w[3], w[2]));
    Some(plugin.unwrap_or_else(|| format!("skill:{}", parts[n - 2])))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Scope;
    use serde_json::json;

    fn item(kind: Kind, name: &str, agents: &[Agent]) -> Augmentation {
        Augmentation {
            id: name.into(),
            name: name.into(),
            kind,
            agents: agents.to_vec(),
            scope: Scope::Global,
            group: "g".into(),
            source: None,
            children: vec![],
            removal: vec![],
        }
    }

    fn call(kind: &str, field: &str, text: &str) -> String {
        json!({"type": "response_item", "payload": {"type": kind, field: text}}).to_string()
    }

    #[test]
    fn claude_counts_loose_skills_and_used_plugins() {
        let json = json!({
            "skillUsage": {
                "tdd": {"usageCount": 3, "lastUsedAt": 10},
                "caveman:caveman": {"usageCount": 1, "lastUsedAt": 20},
                "broken": {"usageCount": 1}
            },
            "pluginUsage": {
                "caveman@caveman": {"usageCount": 5, "lastUsedAt": 30},
                "idle@mp": {"usageCount": 0, "lastUsedAt": 40}
            }
        });
        let mut uses = claude_uses(&json);
        uses.sort();
        assert_eq!(
            uses,
            [
                ("plugin:caveman@caveman".into(), 30),
                ("skill:tdd".into(), 10)
            ]
        );
    }

    #[test]
    fn codex_counts_skill_reads_in_tool_calls_only() {
        let read = "cat /h/.agents/skills/tdd/SKILL.md && sed -n 1p \
                    \"/h/.codex/plugins/cache/mp/review/1.0/skills/lint/SKILL.md\"";
        assert_eq!(
            codex_uses(&call("custom_tool_call", "input", read)),
            ["skill:tdd", "plugin:review@mp"]
        );
        let args = json!({"cmd": "cat ~/.codex/skills/ship/SKILL.md"}).to_string();
        assert_eq!(
            codex_uses(&call("function_call", "arguments", &args)),
            ["skill:ship"]
        );
        let listing = call("message", "input", "/h/.agents/skills/tdd/SKILL.md");
        let not_a_skill = call("custom_tool_call", "input", "cat /h/docs/SKILL.md");
        for line in [listing, not_a_skill, "not json SKILL.md".to_string()] {
            assert!(codex_uses(&line).is_empty());
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

        let usage = read(&env).unwrap();
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
        assert_eq!(read(&env).unwrap(), Usage::default());
        fs::write(home.path().join(".claude.json"), "{").unwrap();
        assert!(read(&env).is_err());
    }
}
