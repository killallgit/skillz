//! Layering guard: modules may import their own layer and the ones below, never above.
//!
//! Layers, lowest first: harness < app < tui < cli. The harness layer is meant to become its
//! own crate, so it may not reach into app, tui or cli. `ALLOWED` lists today's violations as
//! (file, imported top-level module); it may only shrink. An entry that no longer matches a
//! real import fails the test so it gets deleted.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Layer {
    Harness,
    App,
    Tui,
    Cli,
}

const HARNESS: &[&str] = &[
    "action",
    "agents",
    "clone",
    "cost",
    "env",
    "fswalk",
    "hook_text",
    "hooks",
    "install",
    "json_edit",
    "mcp",
    "model",
    "npx_skills",
    "provision",
    "read_optional",
    "redact",
    "runner",
    "skills",
    "subagent_store",
    "subagents",
    "update",
    "usage",
];

const APP: &[&str] = &[
    "apply",
    "info",
    "library",
    "location",
    "matcher",
    "merge",
    "package",
    "profile",
    "prompt",
    "prune",
    "remove",
    "review",
    "snapshot",
    "spinner",
    "stale",
    "store",
    "sync",
    "tree",
    "workspace",
];

fn layer(module: &str) -> Option<Layer> {
    match module {
        "tui" => Some(Layer::Tui),
        "cli" => Some(Layer::Cli),
        m if HARNESS.contains(&m) => Some(Layer::Harness),
        m if APP.contains(&m) => Some(Layer::App),
        _ => None,
    }
}

/// Imports from a higher layer that exist today. Delete a line when its import is fixed.
const ALLOWED: &[(&str, &str)] = &[
    ("agents/claude/marketplace.rs", "profile"),
    ("agents/claude/mcp.rs", "profile"),
    ("agents/claude/mod.rs", "profile"),
    ("agents/codex/mcp.rs", "profile"),
    ("agents/codex/mod.rs", "profile"),
    ("agents/codex/plugins.rs", "profile"),
    ("agents/mod.rs", "profile"),
    ("hooks/provision.rs", "profile"),
    ("install/global.rs", "apply"),
    ("install/global.rs", "profile"),
    ("install/mod.rs", "prompt"),
    ("install/mod.rs", "review"),
    ("install/skills.rs", "matcher"),
    ("install/skills.rs", "prompt"),
    ("provision.rs", "profile"),
    ("subagents.rs", "profile"),
];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn is_test_file(path: &Path) -> bool {
    let name = path.file_name().unwrap().to_string_lossy();
    name == "tests.rs"
        || name == "testutil.rs"
        || name == "test_support.rs"
        || name.ends_with("_tests.rs")
}

/// The code before the first inline `#[cfg(test)] mod name {`, which closes the file. A gated
/// `mod name;` or `use` ends nothing: production code follows it.
fn production(text: &str) -> &str {
    let mut gate_at = None;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with("#[cfg(test)]") {
            gate_at = Some(offset);
        } else if gate_at.is_some() && !trimmed.starts_with("#[") {
            if trimmed.starts_with("mod ") && trimmed.ends_with('{') {
                return &text[..gate_at.unwrap()];
            }
            gate_at = None;
        }
        offset += line.len();
    }
    text
}

/// The first path segment after each `crate::`, looking inside `crate::{a, b::c}` groups.
fn crate_modules(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let ident = |s: &str| {
        s.trim_start()
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect::<String>()
    };
    for (at, _) in text.match_indices("crate::") {
        let after = &text[at + "crate::".len()..];
        let Some(group) = after.strip_prefix('{') else {
            found.insert(ident(after));
            continue;
        };
        let mut depth = 0;
        let mut start = 0;
        for (i, c) in group.char_indices() {
            match c {
                '{' => depth += 1,
                '}' if depth == 0 => {
                    found.insert(ident(&group[start..i]));
                    break;
                }
                '}' => depth -= 1,
                ',' if depth == 0 => {
                    found.insert(ident(&group[start..i]));
                    start = i + 1;
                }
                _ => {}
            }
        }
    }
    found.retain(|m| !m.is_empty());
    found
}

fn violations() -> BTreeSet<(String, String)> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    let mut out = BTreeSet::new();
    for path in files.iter().filter(|p| !is_test_file(p)) {
        let relative = path.strip_prefix(&src).unwrap();
        let module = relative
            .components()
            .next()
            .unwrap()
            .as_os_str()
            .to_string_lossy()
            .trim_end_matches(".rs")
            .to_string();
        if module == "lib" || module == "main" {
            continue;
        }
        let from = layer(&module).unwrap_or_else(|| panic!("give `{module}` a layer"));
        let text = fs::read_to_string(path).unwrap();
        for target in crate_modules(production(&text)) {
            let to = layer(&target).unwrap_or_else(|| panic!("give `{target}` a layer"));
            if to > from {
                out.insert((relative.display().to_string(), target));
            }
        }
    }
    out
}

#[test]
fn no_module_imports_from_a_higher_layer_than_the_allowlist_permits() {
    let found = violations();
    let allowed: BTreeSet<(String, String)> = ALLOWED
        .iter()
        .map(|(f, m)| (f.to_string(), m.to_string()))
        .collect();

    let new: Vec<_> = found.difference(&allowed).collect();
    let fixed: Vec<_> = allowed.difference(&found).collect();

    assert!(new.is_empty(), "imports from a higher layer: {new:#?}");
    assert!(
        fixed.is_empty(),
        "no longer violations, remove from ALLOWED: {fixed:#?}"
    );
}

#[test]
fn group_imports_are_read() {
    let found = crate_modules("use crate::{a::b, c, d::{e, f}};\nuse crate::g;");

    assert_eq!(found.into_iter().collect::<Vec<_>>(), ["a", "c", "d", "g"]);
}

#[test]
fn a_gated_module_declaration_does_not_hide_the_code_after_it() {
    let text = "#[cfg(test)]\nmod testutil;\nuse crate::profile::Spec;\n#[cfg(test)]\nmod tests {\n    use crate::tui::x;\n}\n";

    assert_eq!(
        crate_modules(production(text))
            .into_iter()
            .collect::<Vec<_>>(),
        ["profile"]
    );
}
