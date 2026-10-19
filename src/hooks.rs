use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::Context;
use serde_json::Value;

use crate::action::{Action, Edit, edit_json};
use crate::model::{Agent, Augmentation, Kind, Scope};

/// One hook handler as written in a hooks config file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookEntry {
    pub event: String,
    pub matcher: Option<String>,
    pub command: String,
}

/// Syntax of a config file, decided by its extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Toml,
}

impl Format {
    /// `.toml` files are TOML; everything else is JSON.
    pub fn of(file: &Path) -> Self {
        if file.extension().is_some_and(|e| e == "toml") {
            Format::Toml
        } else {
            Format::Json
        }
    }
}

/// Parse the hooks of a config file. JSON: the top-level `"hooks"` object, nested
/// (Claude/Codex) or flat (Cursor) layout. TOML: the inline hooks of a Codex `config.toml`.
/// A document without hooks has none.
pub fn parse(text: &str, format: Format) -> anyhow::Result<Vec<HookEntry>> {
    let mut out = Vec::new();
    let collect = |e| {
        out.push(e);
        true
    };
    match format {
        Format::Json => {
            let mut doc: Value = serde_json::from_str(text).context("invalid hooks JSON")?;
            walk_json(&mut doc, collect);
        }
        Format::Toml => {
            let mut doc: toml_edit::DocumentMut = text.parse().context("invalid hooks TOML")?;
            walk_toml(&mut doc, collect);
        }
    }
    Ok(out)
}

/// Remove every handler under `event` whose command equals `command`, pruning emptied groups
/// and events. JSON keeps key order; TOML keeps formatting. Returns `text` verbatim when
/// nothing matches.
pub fn remove_hook(
    text: &str,
    format: Format,
    event: &str,
    command: &str,
) -> anyhow::Result<String> {
    let keep = |e: HookEntry| e.event != event || e.command != command;
    match format {
        Format::Json => edit_json(text, |doc| walk_json(doc, keep)),
        Format::Toml => {
            let mut doc: toml_edit::DocumentMut = text.parse().context("invalid TOML")?;
            Ok(if walk_toml(&mut doc, keep) {
                doc.to_string()
            } else {
                text.to_string()
            })
        }
    }
}

/// Visit every handler with a command in `hooks.<Event>[]`: a nested group's `hooks[]`, or
/// the flat entry itself. Drops handlers `keep` rejects, then the groups and events that
/// emptied.
fn walk_json(doc: &mut Value, mut keep: impl FnMut(HookEntry) -> bool) {
    let Some(events) = doc.get_mut("hooks").and_then(Value::as_object_mut) else {
        return;
    };
    events.retain(|event, groups| {
        let Some(groups) = groups.as_array_mut() else {
            return true;
        };
        let before = groups.len();
        groups.retain_mut(|group| {
            let matcher = matcher(group.get("matcher").and_then(Value::as_str));
            let mut keep_handler = |h: &Value| match h.get("command").and_then(Value::as_str) {
                Some(command) => keep(entry(event, &matcher, command)),
                None => true,
            };
            match group.get_mut("hooks").and_then(Value::as_array_mut) {
                Some(handlers) => {
                    let before = handlers.len();
                    handlers.retain(|h| keep_handler(h));
                    !handlers.is_empty() || before == 0
                }
                None => keep_handler(group),
            }
        });
        !groups.is_empty() || before == 0
    });
}

/// Visit every handler in `[[hooks.<Event>]]` + `[[hooks.<Event>.hooks]]` (or the inline-array
/// equivalent). Non-array values under `hooks` (e.g. the `hooks.state` trust table) are not
/// events. Drops handlers `keep` rejects, then the groups and events that emptied; returns
/// whether anything was dropped.
fn walk_toml(doc: &mut toml_edit::DocumentMut, mut keep: impl FnMut(HookEntry) -> bool) -> bool {
    let Some(events) = doc
        .get_mut("hooks")
        .and_then(toml_edit::Item::as_table_like_mut)
    else {
        return false;
    };
    let mut changed = false;
    let mut emptied = Vec::new();
    for (event, groups) in events.iter_mut() {
        let event = event.get().to_string();
        let (before, after) = retain_tables(groups, |group| {
            let matcher = matcher(group.get("matcher").and_then(toml_edit::Item::as_str));
            let Some(handlers) = group.get_mut("hooks") else {
                return true;
            };
            let (b, a) = retain_tables(handlers, |h| {
                match h.get("command").and_then(toml_edit::Item::as_str) {
                    Some(command) => keep(entry(&event, &matcher, command)),
                    None => true,
                }
            });
            changed |= a != b;
            a > 0 || b == 0
        });
        if after == 0 && before > 0 {
            emptied.push(event);
        }
    }
    for event in emptied {
        events.remove(&event);
    }
    changed
}

/// Keep the tables of `[[x]]` or `x = [{..}, ..]` that `keep` approves (it may also edit
/// them); other values stay. Returns the table count before and after.
fn retain_tables(
    item: &mut toml_edit::Item,
    mut keep: impl FnMut(&mut dyn toml_edit::TableLike) -> bool,
) -> (usize, usize) {
    let keeps: Vec<bool> = match item {
        toml_edit::Item::ArrayOfTables(a) => a.iter_mut().map(|t| keep(t)).collect(),
        toml_edit::Item::Value(toml_edit::Value::Array(a)) => a
            .iter_mut()
            .filter_map(toml_edit::Value::as_inline_table_mut)
            .map(|t| keep(t))
            .collect(),
        _ => return (0, 0),
    };
    let mut next = keeps.iter().copied();
    match item {
        toml_edit::Item::ArrayOfTables(a) => a.retain(|_| next.next().unwrap_or(true)),
        toml_edit::Item::Value(toml_edit::Value::Array(a)) => {
            a.retain(|v| !v.is_inline_table() || next.next().unwrap_or(true))
        }
        _ => {}
    }
    (keeps.len(), keeps.iter().filter(|k| **k).count())
}

/// An empty matcher is the same as none.
fn matcher(m: Option<&str>) -> Option<String> {
    m.filter(|m| !m.is_empty()).map(str::to_string)
}

fn entry(event: &str, matcher: &Option<String>, command: &str) -> HookEntry {
    HookEntry {
        event: event.to_string(),
        matcher: matcher.clone(),
        command: command.to_string(),
    }
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
    let tokens = words(command);
    match tokens.iter().find(|t| !is_skippable(t)) {
        // `bash -c "rtk hook"`: the program is inside the quoted script. Recurse only when
        // that splits further, or a space shell_words keeps (U+00A0) would loop forever.
        Some(t) if words(t).len() > 1 => hook_group(t),
        Some(t) => program_group(t),
        None => tokens.first().cloned().unwrap_or_default(),
    }
}

fn words(command: &str) -> Vec<String> {
    shell_words::split(command)
        .unwrap_or_else(|_| command.split_whitespace().map(str::to_string).collect())
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
    let text = match std::fs::read_to_string(file) {
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        other => other.with_context(|| format!("read {}", file.display()))?,
    };
    let entries =
        parse(&text, Format::of(file)).with_context(|| format!("parse {}", file.display()))?;
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
            edit: Edit::RemoveHook {
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
        assert!(!hook_group("rtk\u{a0}hook").is_empty());
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
            parse(&doc.to_string(), Format::Json).unwrap(),
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
            parse(&doc.to_string(), Format::Json).unwrap(),
            vec![
                entry("sessionStart", None, "bd cursor-hook sessionStart"),
                entry("sessionStart", Some("Shell"), "x"),
            ]
        );
    }

    #[test]
    fn no_hooks_key_or_invalid_json() {
        assert!(
            parse(r#"{"enabledPlugins": {}}"#, Format::Json)
                .unwrap()
                .is_empty()
        );
        assert!(parse(r#"{"hooks": []}"#, Format::Json).unwrap().is_empty());
        assert!(parse("{nope", Format::Json).is_err());
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
            edit: Edit::RemoveHook {
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

    const CODEX_TOML: &str = r#"# my config
[features]
codex_hooks = true

[[hooks.SessionStart]]
matcher = "startup|resume"
[[hooks.SessionStart.hooks]]
type = "command"
command = "bd codex-hook SessionStart"
statusMessage = "loading"

[hooks]
Stop = [{ hooks = [{ type = "command", command = "rtk hook codex" }, { type = "command", command = "bd codex-hook Stop" }] }]

[hooks.state."x@y:hooks.json:0"]
trusted = true

[plugins."x@y"]
enabled = true
"#;

    #[test]
    fn parses_toml_layouts_and_skips_state() {
        assert_eq!(
            parse(CODEX_TOML, Format::Toml).unwrap(),
            vec![
                entry(
                    "SessionStart",
                    Some("startup|resume"),
                    "bd codex-hook SessionStart"
                ),
                entry("Stop", None, "rtk hook codex"),
                entry("Stop", None, "bd codex-hook Stop"),
            ]
        );
        assert!(
            parse("[features]\nx = 1\n", Format::Toml)
                .unwrap()
                .is_empty()
        );
        assert!(parse("hooks = 1\n", Format::Toml).unwrap().is_empty());
        assert!(parse("[nope", Format::Toml).is_err());
    }

    #[test]
    fn discover_toml_plans_removal() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        std::fs::write(&file, CODEX_TOML).unwrap();
        let found = discover(&file, Agent::Codex, Scope::Global).unwrap();
        let names: Vec<_> = found.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["bd", "rtk"]);
        let remove = |event: &str, command: &str| Action::Edit {
            file: file.clone(),
            edit: Edit::RemoveHook {
                event: event.into(),
                command: command.into(),
            },
        };
        assert_eq!(
            found[0].removal,
            vec![
                remove("SessionStart", "bd codex-hook SessionStart"),
                remove("Stop", "bd codex-hook Stop"),
            ]
        );
        assert_eq!(found[1].removal, vec![remove("Stop", "rtk hook codex")]);
    }

    /// Removes every parsed entry in turn.
    fn remove_all(text: &str, format: Format) -> String {
        parse(text, format)
            .unwrap()
            .iter()
            .fold(text.to_string(), |t, e| {
                remove_hook(&t, format, &e.event, &e.command).unwrap()
            })
    }

    #[test]
    fn toml_round_trip_removes_hooks_only() {
        let text = remove_all(CODEX_TOML, Format::Toml);
        assert!(parse(&text, Format::Toml).unwrap().is_empty());
        assert!(text.starts_with("# my config\n"));
        let doc: toml_edit::DocumentMut = text.parse().unwrap();
        assert_eq!(doc["features"]["codex_hooks"].as_bool(), Some(true));
        assert_eq!(doc["plugins"]["x@y"]["enabled"].as_bool(), Some(true));
        assert_eq!(
            doc["hooks"]["state"]["x@y:hooks.json:0"]["trusted"].as_bool(),
            Some(true)
        );
        let hooks: Vec<_> = doc["hooks"]
            .as_table_like()
            .unwrap()
            .iter()
            .map(|(k, _)| k)
            .collect();
        assert_eq!(hooks, ["state"]);
    }

    #[test]
    fn json_round_trip_removes_hooks_only() {
        let doc = json!({"z": 1, "hooks": {
            "PreToolUse": [{"matcher": "Bash", "hooks": [
                {"command": "rtk"}, {"type": "prompt", "prompt": "stays"}
            ]}],
            "stop": [{"command": "bd"}, {"command": "x", "matcher": "Shell"}],
            "Stop": [{"hooks": []}]
        }, "m": 2});
        let after: Value =
            serde_json::from_str(&remove_all(&doc.to_string(), Format::Json)).unwrap();
        assert_eq!(
            after,
            json!({"z": 1, "hooks": {
                "PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "prompt", "prompt": "stays"}]}],
                "Stop": [{"hooks": []}]
            }, "m": 2})
        );
    }

    fn json_removed(doc: &Value, event: &str, command: &str) -> Value {
        let out = remove_hook(&doc.to_string(), Format::Json, event, command).unwrap();
        serde_json::from_str(&out).unwrap()
    }

    #[test]
    fn remove_hook_nested_prunes_groups_and_events() {
        let doc = json!({"other": 1, "hooks": {
            "PreToolUse": [
                {"matcher": "Bash", "hooks": [{"command": "rtk"}, {"command": "keep"}]},
                {"matcher": "Edit", "hooks": [{"command": "rtk"}]}
            ],
            "SessionStart": [{"hooks": [{"command": "rtk"}]}],
            "Stop": [{"hooks": []}]
        }});
        let after = json_removed(&doc, "PreToolUse", "rtk");
        assert_eq!(
            after["hooks"]["PreToolUse"],
            json!([{"matcher": "Bash", "hooks": [{"command": "keep"}]}])
        );
        let after = json_removed(&after, "SessionStart", "rtk");
        assert_eq!(
            after,
            json!({"other": 1, "hooks": {
                "PreToolUse": [{"matcher": "Bash", "hooks": [{"command": "keep"}]}],
                "Stop": [{"hooks": []}]
            }})
        );
    }

    #[test]
    fn remove_hook_flat_and_keeps_empty_hooks_object() {
        let doc = json!({"version": 1, "hooks": {"stop": [{"command": "bd"}, {"command": "bd"}]}});
        assert_eq!(
            json_removed(&doc, "stop", "bd"),
            json!({"version": 1, "hooks": {}})
        );
    }

    #[test]
    fn remove_hook_preserves_key_order() {
        let src =
            r#"{"z": 1, "hooks": {"b": [{"command": "x"}], "a": [{"command": "y"}]}, "m": 2}"#;
        let out: Value =
            serde_json::from_str(&remove_hook(src, Format::Json, "b", "x").unwrap()).unwrap();
        let keys: Vec<_> = out.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, ["z", "hooks", "m"]);
        assert_eq!(out["hooks"], json!({"a": [{"command": "y"}]}));
    }

    #[test]
    fn json_remove_hook_absent_is_verbatim() {
        let src = "{\"hooks\":{\"a\":[{\"command\":\"x\"}]},\"plugins\":{}}";
        for src in [src, "{}", r#"{"hooks": 3}"#] {
            for (event, command) in [("missing", "x"), ("a", "other")] {
                assert_eq!(remove_hook(src, Format::Json, event, command).unwrap(), src);
            }
        }
        assert!(remove_hook("{oops", Format::Json, "a", "x").is_err());
    }

    const HOOKS_TOML: &str = r#"[features]
x = 1

# start hooks
[[hooks.SessionStart]]
matcher = "startup"
[[hooks.SessionStart.hooks]]
command = "rtk"
[[hooks.SessionStart.hooks]]
command = "keep" # keep me

[[hooks.SessionStart]]
[[hooks.SessionStart.hooks]]
command = "rtk"

[hooks]
Stop = [{ hooks = [{ command = "rtk" }] }, { hooks = [] }, "odd"]

[hooks.state."a@b:x"]
trusted = true

[plugins."a@b"]
enabled = true
"#;

    fn toml_removed(src: &str, event: &str, command: &str) -> toml_edit::DocumentMut {
        remove_hook(src, Format::Toml, event, command)
            .unwrap()
            .parse()
            .unwrap()
    }

    #[test]
    fn toml_remove_hook_prunes_groups_and_events() {
        let out = remove_hook(HOOKS_TOML, Format::Toml, "SessionStart", "rtk").unwrap();
        assert!(out.contains("# start hooks\n") && out.contains("# keep me"));
        let doc: toml_edit::DocumentMut = out.parse().unwrap();
        let groups = doc["hooks"]["SessionStart"].as_array_of_tables().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups.get(0).unwrap()["matcher"].as_str(), Some("startup"));
        let handlers = groups.get(0).unwrap()["hooks"]
            .as_array_of_tables()
            .unwrap();
        let commands: Vec<_> = handlers.iter().map(|h| h["command"].as_str()).collect();
        assert_eq!(commands, [Some("keep")]);

        let doc = toml_removed(&out, "SessionStart", "keep");
        assert!(doc["hooks"].get("SessionStart").is_none());
        assert_eq!(doc["features"]["x"].as_integer(), Some(1));
        assert_eq!(doc["plugins"]["a@b"]["enabled"].as_bool(), Some(true));
        assert_eq!(
            doc["hooks"]["state"]["a@b:x"]["trusted"].as_bool(),
            Some(true)
        );
    }

    #[test]
    fn toml_remove_hook_inline_keeps_empty_and_odd_entries() {
        let doc = toml_removed(HOOKS_TOML, "Stop", "rtk");
        let stop = doc["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 2);
        assert_eq!(stop.get(1).and_then(|v| v.as_str()), Some("odd"));
    }

    #[test]
    fn toml_remove_hook_absent_is_verbatim() {
        for (event, command) in [("Nope", "rtk"), ("SessionStart", "nope"), ("state", "rtk")] {
            assert_eq!(
                remove_hook(HOOKS_TOML, Format::Toml, event, command).unwrap(),
                HOOKS_TOML
            );
        }
        assert_eq!(
            remove_hook("a = 1\n", Format::Toml, "E", "c").unwrap(),
            "a = 1\n"
        );
        assert!(remove_hook("[nope", Format::Toml, "E", "c").is_err());
    }

    #[test]
    fn format_from_extension() {
        assert_eq!(Format::of(Path::new("a/config.toml")), Format::Toml);
        assert_eq!(Format::of(Path::new("a/hooks.json")), Format::Json);
        assert_eq!(Format::of(Path::new("a/toml")), Format::Json);
    }
}
