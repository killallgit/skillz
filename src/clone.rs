//! Shallow clones of a git source into a temp dir that is removed after use.

use std::path::Path;

use crate::runner::Cmd;

/// Sources reach git/npx/claude as positional args, so a leading `-` would be read as an option.
pub(crate) fn check_source(source: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !source.trim().is_empty() && !source.starts_with('-'),
        "invalid source: {source}"
    );
    Ok(())
}

/// Shallow-clone `source` into a temp dir with `clone`, run `f` on it, and remove the clone,
/// even when `f` panics.
pub(crate) fn with_tmp_clone<T>(
    source: &str,
    clone: impl FnOnce(&Cmd) -> anyhow::Result<()>,
    f: impl FnOnce(&Path) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let tmp = tempfile::Builder::new()
        .prefix("skillz-install-")
        .tempdir()?;
    clone(&clone_command(source, tmp.path()))?;
    f(tmp.path())
}

fn clone_url(source: &str) -> String {
    if source.contains("://") || source.starts_with("git@") {
        source.to_string()
    } else {
        format!("https://github.com/{source}.git")
    }
}

fn clone_command(source: &str, dest: &Path) -> Cmd {
    let args = [
        "clone",
        "--depth",
        "1",
        "--",
        &clone_url(source),
        &dest.to_string_lossy(),
    ]
    .map(String::from)
    .to_vec();
    ("git".into(), args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clone_url_keeps_full_urls() {
        assert_eq!(clone_url("https://x.dev/a/b.git"), "https://x.dev/a/b.git");
        assert_eq!(
            clone_url("git@github.com:a/b.git"),
            "git@github.com:a/b.git"
        );
        assert_eq!(clone_url("a/b"), "https://github.com/a/b.git");
    }

    #[test]
    fn option_looking_and_blank_sources_are_refused() {
        // both would be read by git/npx as options or nothing
        assert!(check_source("-x").is_err() && check_source(" ").is_err());
        assert!(check_source("a/b").is_ok());
    }
}
