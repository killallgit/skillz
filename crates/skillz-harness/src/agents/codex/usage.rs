use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::UNIX_EPOCH;

use serde_json::Value;

use super::timestamp::record_ms;
use crate::env::Env;

/// Every skill or plugin use found in the session logs, as `(key, ms)`.
pub(super) fn all_uses(env: &Env) -> Vec<(String, u64)> {
    let mut uses = vec![];
    for dir in ["sessions", "archived_sessions"] {
        let pattern = glob::Pattern::escape(&super::dir(env).join(dir).to_string_lossy());
        for log in glob::glob(&format!("{pattern}/**/*.jsonl"))
            .into_iter()
            .flatten()
            .flatten()
        {
            read_log(&log, &mut uses);
        }
    }
    uses
}

/// A use is dated by its record's own `timestamp`; the log's mtime (its last write) stands in
/// for a record without one.
fn read_log(log: &Path, uses: &mut Vec<(String, u64)>) {
    let Ok(file) = fs::File::open(log) else {
        return;
    };
    let mtime = mtime_ms(&file);
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if line.contains("SKILL.md") {
            let ms = record_ms(&line).unwrap_or(mtime);
            uses.extend(codex_uses(&line).into_iter().map(|key| (key, ms)));
        }
    }
}

fn mtime_ms(file: &fs::File) -> u64 {
    file.metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as u64)
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
        (true, Some(input)) => skill_paths(input).flat_map(use_keys).collect(),
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

fn use_keys(path: &str) -> Vec<String> {
    let parts: Vec<&str> = path.split('/').collect();
    let n = parts.len();
    if n < 3 || parts[n - 3] != "skills" {
        return vec![];
    }
    let skill = parts[n - 2];
    match parts
        .windows(4)
        .find(|w| w[0] == "plugins" && w[1] == "cache")
    {
        Some(w) => vec![format!("plugin:{}@{}", w[3], w[2])],
        None => vec![format!("skill:{skill}")],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(kind: &str, field: &str, text: &str) -> String {
        serde_json::json!({"type": "response_item", "payload": {"type": kind, field: text}})
            .to_string()
    }

    #[test]
    fn usage_counts_skill_reads_in_tool_calls_only() {
        let read = "cat /h/.agents/skills/tdd/SKILL.md && sed -n 1p \
                    \"/h/.codex/plugins/cache/mp/review/1.0/skills/lint/SKILL.md\"";
        assert_eq!(
            codex_uses(&call("custom_tool_call", "input", read)),
            ["skill:tdd", "plugin:review@mp"]
        );
        let args = serde_json::json!({"cmd": "cat ~/.codex/skills/ship/SKILL.md"}).to_string();
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
}
