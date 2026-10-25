use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::harness::hook_text::{Format, HookEntry, parse};

const HOOK_FILES: [&str; 2] = ["hooks/hooks.json", "hooks.json"];

/// The hooks a plugin declares itself: its `hooks.json` files and the `hooks` of its manifest
/// (a path, a list of paths, or the hooks inline). Anything unreadable declares nothing.
pub fn declared(root: &Path, manifest: &Path) -> Vec<HookEntry> {
    let mut files: Vec<PathBuf> = HOOK_FILES.iter().map(|f| root.join(f)).collect();
    let mut found = Vec::new();
    match read_json(manifest).and_then(|m| m.get("hooks").cloned()) {
        Some(Value::String(path)) => files.push(root.join(path)),
        Some(Value::Array(paths)) => {
            files.extend(paths.iter().filter_map(Value::as_str).map(|p| root.join(p)))
        }
        Some(inline @ Value::Object(_)) => found.extend(entries(&wrapped(inline))),
        _ => {}
    }
    for file in files {
        if let Some(doc) = read_json(&file) {
            found.extend(entries(&doc));
        }
    }
    let mut unique = Vec::new();
    for entry in found {
        if !unique.contains(&entry) {
            unique.push(entry);
        }
    }
    unique
}

fn read_json(file: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(file).ok()?).ok()
}

fn wrapped(inline: Value) -> Value {
    match inline.get("hooks") {
        Some(_) => inline,
        None => json!({ "hooks": inline }),
    }
}

fn entries(doc: &Value) -> Vec<HookEntry> {
    parse(&doc.to_string(), Format::Json).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const STOP: &str =
        r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"say done"}]}]}}"#;

    fn write(root: &Path, file: &str, text: &str) {
        let path = root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn commands(root: &Path) -> Vec<String> {
        let manifest = root.join(".claude-plugin/plugin.json");
        declared(root, &manifest)
            .into_iter()
            .map(|h| h.command)
            .collect()
    }

    #[test]
    fn hooks_json_hooks_are_declared() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "hooks/hooks.json", STOP);

        let found = commands(dir.path());

        assert_eq!(found, ["say done"]);
    }

    #[test]
    fn a_manifest_names_hook_files_by_path_or_list() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "extra.json", STOP);
        write(
            dir.path(),
            "more.json",
            &STOP.replace("say done", "say more"),
        );
        for (manifest, expected) in [
            (r#"{"hooks":"./extra.json"}"#, 1),
            (r#"{"hooks":["extra.json","more.json"]}"#, 2),
        ] {
            write(dir.path(), ".claude-plugin/plugin.json", manifest);

            let found = commands(dir.path());

            assert_eq!(found.len(), expected);
        }
    }

    #[test]
    fn a_manifest_may_declare_hooks_inline_with_or_without_the_hooks_key() {
        let dir = tempfile::tempdir().unwrap();
        let bare = r#"{"hooks":{"Stop":[{"hooks":[{"command":"say done"}]}]}}"#;
        let keyed = r#"{"hooks":{"hooks":{"Stop":[{"hooks":[{"command":"say done"}]}]}}}"#;
        for manifest in [bare, keyed] {
            write(dir.path(), ".claude-plugin/plugin.json", manifest);

            let found = commands(dir.path());

            assert_eq!(found, ["say done"]);
        }
    }

    #[test]
    fn the_same_hook_declared_twice_counts_once() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "hooks/hooks.json", STOP);
        write(
            dir.path(),
            ".claude-plugin/plugin.json",
            r#"{"hooks":"hooks/hooks.json"}"#,
        );

        let found = commands(dir.path());

        assert_eq!(found.len(), 1);
    }

    #[test]
    fn unreadable_declarations_declare_nothing() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "hooks/hooks.json", "{");
        write(dir.path(), ".claude-plugin/plugin.json", "[");

        let found = commands(dir.path());

        assert!(found.is_empty());
    }
}
