use std::path::Path;

/// Programs that run another program; the hook belongs to what they run.
const WRAPPERS: &[&str] = &[
    "bash", "sh", "zsh", "fish", "node", "python", "python3", "uv", "uvx", "bun", "bunx", "deno",
    "npx", "pnpx", "pnpm", "yarn", "npm", "exec", "env", "sudo", "command", "run", "nohup", "time",
    "if", "then", "test", "[", "[[", "!",
];

/// Hidden home dirs that belong to the OS, not to a hook-installing tool. Agents add their own
/// (`GroupHints::agent_dirs`).
const SHARED_DIRS: &[&str] = &["config", "local"];

/// What the installed agents contribute to grouping: where they keep plugin files and which
/// hidden home dirs are theirs rather than a hook-installing tool's.
#[derive(Debug, Default, Clone)]
pub struct GroupHints {
    /// Env vars a plugin's hook commands use for the plugin's own dir.
    pub plugin_root_vars: Vec<&'static str>,
    /// Hidden home dir names (`.claude` -> `claude`) agents own.
    pub agent_dirs: Vec<&'static str>,
}

/// Name of the tool a hook command belongs to, so related hooks group together.
/// e.g. `bd prime --hook-json` -> `bd`, `bash '/x/.claude/hooks/herdr-agent-state.sh' s` -> `herdr-agent-state`.
pub fn hook_group(command: &str, hints: &GroupHints) -> String {
    let tokens = words(command);
    match tokens.iter().find(|t| !is_skippable(t)) {
        // `bash -c "rtk hook"`: the program is inside the quoted script. Recurse only when
        // that splits further, or a space shell_words keeps (U+00A0) would loop forever.
        Some(t) if words(t).len() > 1 => hook_group(t, hints),
        Some(t) => program_group(t, hints),
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

fn program_group(token: &str, hints: &GroupHints) -> String {
    if hints
        .plugin_root_vars
        .iter()
        .any(|var| token.contains(&format!("${{{var}}}")))
    {
        return "plugin".to_string();
    }
    let path = Path::new(token);
    if let Some(tool) = tool_dir(path, &hints.agent_dirs) {
        return tool;
    }
    match path.file_stem().and_then(|s| s.to_str()) {
        Some(stem) if token.contains('/') || path.extension().is_some() => stem.to_string(),
        _ => token.to_string(),
    }
}

/// `/home/me/.orca/hooks/x.sh` -> `orca`: the first hidden directory not shared by agents.
fn tool_dir(path: &Path, agent_dirs: &[&str]) -> Option<String> {
    let parent = path.parent()?;
    parent
        .components()
        .filter_map(|c| c.as_os_str().to_str()?.strip_prefix('.'))
        .find(|name| {
            !name.is_empty()
                && !name.starts_with('.')
                && !SHARED_DIRS.contains(name)
                && !agent_dirs.contains(name)
        })
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hook_group(command: &str) -> String {
        let hints = GroupHints {
            plugin_root_vars: vec!["CLAUDE_PLUGIN_ROOT"],
            agent_dirs: vec!["claude", "codex", "agents", "cursor"],
        };
        super::hook_group(command, &hints)
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
}
