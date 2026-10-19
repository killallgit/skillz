use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::Context;
use serde_json::Value;

use crate::action::{Action, Edit};
use crate::model::{Agent, Augmentation, Kind, Scope};

/// One hook handler as written in a hooks config file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookEntry {
    pub event: String,
    pub matcher: Option<String>,
    pub command: String,
}

/// Parse a hooks JSON document (nested Claude/Codex layout or flat Cursor layout).
/// Reads the top-level `"hooks"` object; a document without it has no hooks.
pub fn parse_hooks(json: &str) -> anyhow::Result<Vec<HookEntry>> {
    let doc: Value = serde_json::from_str(json).context("invalid hooks JSON")?;
    let Some(events) = doc.get("hooks").and_then(Value::as_object) else {
        return Ok(Vec::new());
    };
    Ok(events
        .iter()
        .flat_map(|(event, groups)| {
            groups
                .as_array()
                .into_iter()
                .flatten()
                .map(move |g| (event, g))
        })
        .flat_map(|(event, group)| group_entries(event, group))
        .collect())
}

/// Handlers of one event entry: a nested group's `hooks[]`, or the flat entry itself.
fn group_entries(event: &str, group: &Value) -> Vec<HookEntry> {
    let matcher = group
        .get("matcher")
        .and_then(Value::as_str)
        .filter(|m| !m.is_empty())
        .map(str::to_string);
    let handlers = match group.get("hooks").and_then(Value::as_array) {
        Some(nested) => nested.iter().collect(),
        None => vec![group],
    };
    handlers
        .into_iter()
        .filter_map(|h| h.get("command").and_then(Value::as_str))
        .map(|command| HookEntry {
            event: event.to_string(),
            matcher: matcher.clone(),
            command: command.to_string(),
        })
        .collect()
}

/// Programs that run another program; the hook belongs to what they run.
const WRAPPERS: &[&str] = &[
    "bash", "sh", "zsh", "fish", "node", "python", "python3", "uv", "uvx", "bun", "bunx", "deno",
    "npx", "pnpx", "pnpm", "yarn", "npm", "exec", "env", "sudo", "command", "run", "nohup", "time",
    "if", "then", "test", "[", "[[", "!",
];

/// Hidden home dirs that belong to agents or the OS, not to a hook-installing tool.
const SHARED_DIRS: &[&str] = &["claude", "codex", "cursor", "agents", "config", "local"];

/// Name of the tool a hook command belongs to, so related hooks group together.
/// e.g. `bd prime --hook-json` -> `bd`, `bash '/x/.claude/hooks/herdr-agent-state.sh' s` -> `herdr-agent-state`.
pub fn hook_group(command: &str) -> String {
    let tokens = shell_words::split(command)
        .unwrap_or_else(|_| command.split_whitespace().map(str::to_string).collect());
    match tokens.iter().find(|t| !is_skippable(t)) {
        // `bash -c "rtk hook"`: the program is inside the quoted script.
        Some(t) if t.contains(char::is_whitespace) => hook_group(t),
        Some(t) => program_group(t),
        None => tokens.first().cloned().unwrap_or_default(),
    }
}

fn is_skippable(token: &str) -> bool {
    let name = Path::new(token)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(token);
    token.starts_with('-') || is_assignment(token) || WRAPPERS.contains(&name)
}

fn is_assignment(token: &str) -> bool {
    token.split_once('=').is_some_and(|(var, _)| {
        !var.is_empty() && var.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

fn program_group(token: &str) -> String {
    if token.contains("${CLAUDE_PLUGIN_ROOT}") {
        return "plugin".to_string();
    }
    let path = Path::new(token);
    if let Some(tool) = tool_dir(path) {
        return tool;
    }
    match path.file_stem().and_then(|s| s.to_str()) {
        Some(stem) if token.contains('/') || path.extension().is_some() => stem.to_string(),
        _ => token.to_string(),
    }
}

/// `/home/me/.orca/hooks/x.sh` -> `orca`: the first hidden directory not shared by agents.
fn tool_dir(path: &Path) -> Option<String> {
    let parent = path.parent()?;
    parent
        .components()
        .filter_map(|c| c.as_os_str().to_str()?.strip_prefix('.'))
        .find(|name| !name.is_empty() && !name.starts_with('.') && !SHARED_DIRS.contains(name))
        .map(str::to_string)
}

/// Hooks in one config file, one `Augmentation` per `hook_group`, each with its removal plan.
/// A missing file yields an empty list.
pub fn discover(file: &Path, agent: Agent, scope: Scope) -> anyhow::Result<Vec<Augmentation>> {
    let json = match std::fs::read_to_string(file) {
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        other => other.with_context(|| format!("read {}", file.display()))?,
    };
    let entries = parse_hooks(&json).with_context(|| format!("parse {}", file.display()))?;
    let mut groups: BTreeMap<String, Vec<HookEntry>> = BTreeMap::new();
    for entry in entries {
        groups
            .entry(hook_group(&entry.command))
            .or_default()
            .push(entry);
    }
    Ok(groups
        .into_iter()
        .map(|(name, entries)| augmentation(file, agent, scope, name, &entries))
        .collect())
}

fn augmentation(
    file: &Path,
    agent: Agent,
    scope: Scope,
    name: String,
    entries: &[HookEntry],
) -> Augmentation {
    let mut removal: Vec<Action> = Vec::new();
    for e in entries {
        let action = Action::Edit {
            file: file.to_path_buf(),
            edit: Edit::JsonRemoveHook {
                event: e.event.clone(),
                command: e.command.clone(),
            },
        };
        if !removal.contains(&action) {
            removal.push(action);
        }
    }
    Augmentation {
        id: format!("hook:{}:{}", file.display(), name),
        kind: Kind::Hook,
        agents: vec![agent],
        scope,
        group: "hooks".to_string(),
        source: None,
        children: entries.iter().map(child_label).collect(),
        removal,
        name,
    }
}

fn child_label(e: &HookEntry) -> String {
    match &e.matcher {
        Some(m) => format!("{} [{}]: {}", e.event, m, e.command),
        None => format!("{}: {}", e.event, e.command),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry(event: &str, matcher: Option<&str>, command: &str) -> HookEntry {
        HookEntry {
            event: event.into(),
            matcher: matcher.map(Into::into),
            command: command.into(),
        }
    }

    #[test]
    fn groups_real_commands() {
        let cases = [
            ("bd prime --hook-json", "bd"),
            ("rtk hook claude", "rtk"),
            (
                "bash '/Users/ryan/.claude/hooks/herdr-agent-state.sh' session",
                "herdr-agent-state",
            ),
            (
                "/Users/ryan/.superset/hooks/cursor-hook.sh PermissionRequest",
                "superset",
            ),
            (
                "if [ -f '/Users/ryan/.orca/agent-hooks/cursor-hook.sh' ] && [ -r '/Users/ryan/.orca/agent-hooks/cursor-hook.sh' ]; then ORCA_X='{}' /bin/sh '/Users/ryan/.orca/agent-hooks/cursor-hook.sh'; fi",
                "orca",
            ),
            ("node \"${CLAUDE_PLUGIN_ROOT}/hooks/x.js\"", "plugin"),
            ("FOO=1 uv run python script.py", "script"),
            ("npx -y some-tool run", "some-tool"),
        ];
        for (cmd, group) in cases {
            assert_eq!(hook_group(cmd), group, "{cmd}");
        }
    }

    #[test]
    fn groups_edge_commands() {
        assert_eq!(hook_group("bash -c \"rtk hook claude\""), "rtk");
        assert_eq!(hook_group("/usr/bin/env bd sync"), "bd");
        assert_eq!(hook_group("~/.config/tool/run.sh"), "run");
        assert_eq!(hook_group("bd 'unterminated"), "bd");
        assert_eq!(hook_group("bash"), "bash");
        assert_eq!(hook_group(""), "");
        assert_eq!(hook_group("=x ok"), "=x");
    }

    #[test]
    fn parses_nested_layout() {
        let doc = json!({"hooks": {
            "PreToolUse": [{"matcher": "Bash", "hooks": [
                {"type": "command", "command": "rtk hook claude"},
                {"type": "prompt", "prompt": "no command"}
            ]}],
            "SessionStart": [{"matcher": "", "hooks": [{"type": "command", "command": "bd prime"}]}]
        }});
        assert_eq!(
            parse_hooks(&doc.to_string()).unwrap(),
            vec![
                entry("PreToolUse", Some("Bash"), "rtk hook claude"),
                entry("SessionStart", None, "bd prime"),
            ]
        );
    }

    #[test]
    fn parses_flat_layout() {
        let doc = json!({"version": 1, "hooks": {"sessionStart": [
            {"command": "bd cursor-hook sessionStart"},
            {"command": "x", "matcher": "Shell"}
        ], "bogus": {"not": "an array"}}});
        assert_eq!(
            parse_hooks(&doc.to_string()).unwrap(),
            vec![
                entry("sessionStart", None, "bd cursor-hook sessionStart"),
                entry("sessionStart", Some("Shell"), "x"),
            ]
        );
    }

    #[test]
    fn no_hooks_key_or_invalid_json() {
        assert!(parse_hooks(r#"{"enabledPlugins": {}}"#).unwrap().is_empty());
        assert!(parse_hooks(r#"{"hooks": []}"#).unwrap().is_empty());
        assert!(parse_hooks("{nope").is_err());
    }

    #[test]
    fn discover_missing_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let found = discover(&dir.path().join("none.json"), Agent::Claude, Scope::Global).unwrap();
        assert!(found.is_empty());
    }

    #[test]
    fn discover_invalid_file_errors() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        std::fs::write(&file, "{").unwrap();
        assert!(discover(&file, Agent::Claude, Scope::Global).is_err());
        assert!(discover(dir.path(), Agent::Claude, Scope::Global).is_err());
    }

    #[test]
    fn discover_groups_and_plans_removal() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        let doc = json!({"hooks": {
            "SessionStart": [{"hooks": [{"command": "bd prime"}, {"command": "rtk hook claude"}]}],
            "PreCompact": [{"hooks": [{"command": "bd prime"}]}],
            "PreToolUse": [
                {"matcher": "Bash", "hooks": [{"command": "rtk hook claude"}]},
                {"matcher": "Edit", "hooks": [{"command": "rtk hook claude"}]}
            ]
        }});
        std::fs::write(&file, doc.to_string()).unwrap();

        let found = discover(&file, Agent::Codex, Scope::Project).unwrap();
        let names: Vec<_> = found.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["bd", "rtk"]);

        let rtk = &found[1];
        assert_eq!(rtk.id, format!("hook:{}:rtk", file.display()));
        assert_eq!(rtk.kind, Kind::Hook);
        assert_eq!(rtk.agents, vec![Agent::Codex]);
        assert_eq!(rtk.scope, Scope::Project);
        assert_eq!(rtk.group, "hooks");
        assert_eq!(rtk.source, None);
        assert_eq!(rtk.children.len(), 3);
        let remove = |event: &str| Action::Edit {
            file: file.clone(),
            edit: Edit::JsonRemoveHook {
                event: event.into(),
                command: "rtk hook claude".into(),
            },
        };
        assert_eq!(
            rtk.removal,
            vec![remove("SessionStart"), remove("PreToolUse")]
        );
        assert_eq!(found[0].removal.len(), 2);
    }

    #[test]
    fn discover_removal_empties_hooks() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("hooks.json");
        let doc = json!({"version": 1, "hooks": {
            "sessionStart": [{"command": "bd cursor-hook sessionStart"}],
            "stop": [{"command": "/x/.orca/h.sh stop"}, {"command": "bd cursor-hook stop"}]
        }});
        std::fs::write(&file, doc.to_string()).unwrap();
        let plan: Vec<Action> = discover(&file, Agent::Cursor, Scope::Global)
            .unwrap()
            .into_iter()
            .flat_map(|a| a.removal)
            .collect();
        crate::action::execute(&plan).unwrap();
        let after: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(after, json!({"version": 1, "hooks": {}}));
    }
}
