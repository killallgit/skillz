use anyhow::Context;
use serde_json::Value;

/// Parse, mutate, re-serialize. An edit that changes nothing returns `contents` verbatim so
/// callers can detect no-ops by comparing text.
pub(crate) fn edit_json(contents: &str, mutate: impl FnOnce(&mut Value)) -> anyhow::Result<String> {
    let original: Value = serde_json::from_str(contents).context("invalid JSON")?;
    let mut doc = original.clone();
    mutate(&mut doc);
    if doc == original {
        return Ok(contents.to_string());
    }
    Ok(serde_json::to_string_pretty(&doc)? + "\n")
}
