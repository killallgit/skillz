use std::ffi::OsStr;
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use anyhow::Context;
use serde::Serialize;

/// A path strictly inside an owned root, built from plain segments. The only way to get a
/// `RemovePath`, so every deletion passes the same rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RemovalTarget {
    root: PathBuf,
    path: PathBuf,
}

impl RemovalTarget {
    /// `root` joined with `segments`, or `None` unless there is at least one segment and each
    /// is one plain UTF-8 name: non-empty, not `.`/`..`, no `/` or `\`. Config-supplied names
    /// go in as separate segments; a `&Path` iterates as its components.
    pub fn new<S: AsRef<OsStr>>(
        root: &Path,
        segments: impl IntoIterator<Item = S>,
    ) -> Option<Self> {
        let mut path = root.to_path_buf();
        for seg in segments {
            let seg = seg.as_ref().to_str()?;
            if seg.is_empty() || seg == "." || seg == ".." || seg.contains(['/', '\\']) {
                return None;
            }
            path.push(seg);
        }
        let target = Self {
            root: root.to_path_buf(),
            path,
        };
        target.inside().then_some(target)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn inside(&self) -> bool {
        self.path.strip_prefix(&self.root).is_ok_and(|rest| {
            rest.components().next().is_some()
                && rest.components().all(|c| matches!(c, Component::Normal(_)))
        })
    }
}

/// Removes a symlink itself (never its target), a file, or a whole directory tree. Returns
/// false (removes nothing) when the target is not strictly inside its root, or an ancestor is
/// a symlink, as deleting would reach into its target.
pub(super) fn remove_path(target: &RemovalTarget) -> anyhow::Result<bool> {
    let path = &target.path;
    if !target.inside() || under_symlinked_ancestor(path) {
        return Ok(false);
    }
    let meta = match fs::symlink_metadata(path) {
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
        other => other.with_context(|| format!("stat {}", path.display()))?,
    };
    if meta.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
    .with_context(|| format!("remove {}", path.display()))?;
    Ok(true)
}

/// Only the two nearest ancestors count (managed root..target, e.g. cache/<mp>/<name>), so
/// system links (macOS /var) and a symlinked ~/.claude don't block removal.
fn under_symlinked_ancestor(path: &Path) -> bool {
    let linked = |a: &Path| fs::symlink_metadata(a).is_ok_and(|m| m.file_type().is_symlink());
    path.ancestors().skip(1).take(2).any(linked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::action::testutil::*;
    use crate::harness::action::{Action, execute};
    use crate::harness::runner::NoCommands;

    #[test]
    fn removal_target_accepts_only_plain_segments() {
        let root = Path::new("/r");
        for bad in ["", ".", "..", "a/b", "a\\b", "/abs"] {
            assert!(RemovalTarget::new(root, [bad]).is_none(), "{bad}");
            assert!(RemovalTarget::new(root, ["ok", bad]).is_none(), "{bad}");
        }
        assert!(RemovalTarget::new(root, Vec::<&str>::new()).is_none());
        assert!(RemovalTarget::new(root, Path::new("a/../b")).is_none());
        for good in ["a..b", ".hidden"] {
            let target = RemovalTarget::new(root, [good]).unwrap();
            assert_eq!(target.path(), root.join(good));
        }
        let nested = RemovalTarget::new(root, Path::new("a/b")).unwrap();
        assert_eq!(nested.path(), Path::new("/r/a/b"));
    }

    #[test]
    fn execute_skips_target_outside_its_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        let victim = dir.path().join("victim");
        fs::create_dir(&root).unwrap();
        write(&victim, "x");
        let escaped = [
            RemovalTarget {
                root: root.clone(),
                path: victim.clone(),
            },
            RemovalTarget {
                root: root.clone(),
                path: root.join("../victim"),
            },
            RemovalTarget {
                root: root.clone(),
                path: root.clone(),
            },
        ];
        let report = execute(&escaped.map(Action::RemovePath), &NoCommands).unwrap();
        assert!(report.removed.is_empty());
        assert!(victim.exists() && root.exists());
    }

    #[test]
    fn execute_removes_paths_and_skips_missing() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f.txt");
        let tree = dir.path().join("tree");
        write(&file, "x");
        fs::create_dir_all(tree.join("a/b")).unwrap();
        write(&tree.join("a/b/c"), "x");
        let missing = dir.path().join("missing");

        let report = execute(
            &[rm(&file), rm(&missing), rm(&tree), rm(&file)],
            &NoCommands,
        )
        .unwrap();
        assert_eq!(report.removed, vec![file.clone(), tree.clone()]);
        assert!(report.edited.is_empty());
        assert!(!file.exists() && !tree.exists());
    }

    #[cfg(unix)]
    #[test]
    fn execute_removes_symlink_not_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("source-checkout");
        fs::create_dir(&target).unwrap();
        write(&target.join("SKILL.md"), "x");
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let dangling = dir.path().join("dangling");
        std::os::unix::fs::symlink(dir.path().join("gone"), &dangling).unwrap();

        let report = execute(&[rm(&link), rm(&dangling)], &NoCommands).unwrap();
        assert_eq!(report.removed, vec![link.clone(), dangling.clone()]);
        assert!(fs::symlink_metadata(&link).is_err());
        assert!(fs::symlink_metadata(&dangling).is_err());
        assert!(target.join("SKILL.md").exists());
    }

    #[cfg(unix)]
    #[test]
    fn execute_skips_path_under_symlinked_ancestor() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("checkout");
        fs::create_dir_all(real.join("name")).unwrap();
        write(&real.join("name/SKILL.md"), "x");
        fs::create_dir(dir.path().join("cache")).unwrap();
        std::os::unix::fs::symlink(&real, dir.path().join("cache/mp")).unwrap();

        let report = execute(&[rm(&dir.path().join("cache/mp/name"))], &NoCommands).unwrap();
        assert!(report.removed.is_empty());
        assert!(real.join("name/SKILL.md").exists());
    }
}
