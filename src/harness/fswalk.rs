//! The one recursive directory walk used to scan a fetched source for skills and subagents.

use std::fs;
use std::path::Path;

use anyhow::Context;

const SKIPPED_DIRS: [&str; 6] = [
    ".git",
    "node_modules",
    "dist",
    "build",
    "target",
    "__pycache__",
];

/// Call `visit(path, is_dir)` for `root` and everything under it, skipping VCS and build dirs.
/// Symlinks are never followed, so a link loop cannot recurse forever.
pub fn walk(root: &Path, visit: &mut dyn FnMut(&Path, bool)) -> anyhow::Result<()> {
    visit(root, true);
    walk_children(root, visit)
}

fn walk_children(dir: &Path, visit: &mut dyn FnMut(&Path, bool)) -> anyhow::Result<()> {
    let entries = fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_dir() {
            if !is_skipped(&entry.file_name()) {
                visit(&path, true);
                walk_children(&path, visit)?;
            }
        } else if kind.is_file() {
            visit(&path, false);
        }
    }
    Ok(())
}

fn is_skipped(name: &std::ffi::OsStr) -> bool {
    name.to_str().is_some_and(|n| SKIPPED_DIRS.contains(&n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visits_files_and_dirs_but_not_skipped_dirs() {
        let dir = tempfile::tempdir().unwrap();
        for rel in [
            "a/b.txt",
            ".git/x",
            "node_modules/y",
            "target/z",
            "c/d/e.txt",
        ] {
            let path = dir.path().join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "").unwrap();
        }
        let mut seen = vec![];
        walk(dir.path(), &mut |path, is_dir| {
            let rel = path.strip_prefix(dir.path()).unwrap();
            seen.push((rel.to_string_lossy().into_owned(), is_dir));
        })
        .unwrap();
        seen.sort();
        let expected: Vec<(String, bool)> = [
            ("", true),
            ("a", true),
            ("a/b.txt", false),
            ("c", true),
            ("c/d", true),
            ("c/d/e.txt", false),
        ]
        .map(|(p, d)| (p.to_string(), d))
        .to_vec();
        assert_eq!(seen, expected);
    }

    #[test]
    fn a_missing_root_is_an_error() {
        assert!(walk(Path::new("/definitely/not/here"), &mut |_, _| {}).is_err());
    }
}
