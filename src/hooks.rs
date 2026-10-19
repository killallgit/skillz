use std::path::Path;

use crate::model::{Agent, Augmentation, Scope};

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
    let _ = json;
    todo!("lane: config-core")
}

/// Name of the tool a hook command belongs to, so related hooks group together.
/// e.g. `bd prime --hook-json` -> `bd`, `bash '/x/.claude/hooks/herdr-agent-state.sh' s` -> `herdr-agent-state`.
pub fn hook_group(command: &str) -> String {
    let _ = command;
    todo!("lane: config-core")
}

/// Hooks in one config file, one `Augmentation` per `hook_group`, each with its removal plan.
/// A missing file yields an empty list.
pub fn discover(file: &Path, agent: Agent, scope: Scope) -> anyhow::Result<Vec<Augmentation>> {
    let _ = (file, agent, scope);
    todo!("lane: config-core")
}
