//! Shallow clones of a git source into a temp dir that is removed after use.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::runner::Cmd;

/// Shallow-clone `source` into a temp dir with `clone`, run `f` on it, and remove the clone.
pub(crate) fn with_tmp_clone<T>(
    source: &str,
    clone: impl FnOnce(&Cmd) -> anyhow::Result<()>,
    f: impl FnOnce(&Path) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let tmp = std::env::temp_dir().join(unique_name());
    let result = clone(&clone_command(source, &tmp)).and_then(|()| f(&tmp));
    let _ = std::fs::remove_dir_all(&tmp);
    result
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
        &clone_url(source),
        &dest.to_string_lossy(),
    ]
    .map(String::from)
    .to_vec();
    ("git".into(), args)
}

fn unique_name() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("skillz-install-{}-{nanos}", std::process::id())
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
}
