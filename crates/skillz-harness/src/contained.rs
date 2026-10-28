//! Whether a source tree's symlinks stay inside it. npx, claude and codex copy what a skill or
//! plugin directory links to, so a link that leaves the source would carry the user's own files
//! into an install, and one that only passes outside could be repointed between check and copy.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::Context;

/// More symlinks than this while resolving one path is a loop, as the OS treats it.
const MAX_HOPS: usize = 40;

/// Symlinks under a source tree that leave it. One per scan; memoised by canonical directory.
pub struct Containment {
    root: PathBuf,
    /// The real directories each scanned directory's entries reach; `None` for a directory with an
    /// entry that leaves the root.
    reached: HashMap<PathBuf, Option<Vec<PathBuf>>>,
}

impl Containment {
    pub fn new(root: &Path) -> anyhow::Result<Containment> {
        let root = root
            .canonicalize()
            .with_context(|| format!("reading {}", root.display()))?;
        Ok(Containment {
            root,
            reached: HashMap::new(),
        })
    }

    /// Whether `dir` or anything under it is a symlink that leaves the root at any hop, dangles,
    /// or links to a directory that does.
    pub fn escapes(&mut self, dir: &Path) -> anyhow::Result<bool> {
        let Some(start) = self.resolve(&self.root, dir)? else {
            return Ok(true);
        };
        let mut seen = HashSet::new();
        let mut pending = vec![start];
        while let Some(dir) = pending.pop() {
            if !seen.insert(dir.clone()) {
                continue;
            }
            match self.reached_from(&dir)? {
                Some(dirs) => pending.extend(dirs.iter().cloned()),
                None => return Ok(true),
            }
        }
        Ok(false)
    }

    fn reached_from(&mut self, dir: &Path) -> anyhow::Result<&Option<Vec<PathBuf>>> {
        if !self.reached.contains_key(dir) {
            let reached = self.scan(dir)?;
            self.reached.insert(dir.to_path_buf(), reached);
        }
        Ok(&self.reached[dir])
    }

    fn scan(&self, dir: &Path) -> anyhow::Result<Option<Vec<PathBuf>>> {
        let mut dirs = vec![];
        let entries =
            std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))?;
        for entry in entries {
            match self.resolve(dir, Path::new(&entry?.file_name()))? {
                Some(real) if real.is_dir() => dirs.push(real),
                Some(_) => {}
                None => return Ok(None),
            }
        }
        Ok(Some(dirs))
    }

    /// Where `path`, taken from the real directory `from`, really is, following one symlink at a
    /// time as the OS does. `None` when a step lands outside the root other than on the way down
    /// to it, a link dangles, or the links loop.
    fn resolve(&self, from: &Path, path: &Path) -> anyhow::Result<Option<PathBuf>> {
        let mut real = from.to_path_buf();
        let mut steps = reversed_steps(path);
        let mut hops = 0;
        while let Some(step) = steps.pop() {
            if step == "." {
                continue;
            }
            if step == ".." {
                real.pop();
                continue;
            }
            let next = real.join(&step);
            let kind = match std::fs::symlink_metadata(&next) {
                Ok(metadata) => metadata.file_type(),
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                    ) =>
                {
                    return Ok(None);
                }
                Err(error) => {
                    return Err(error).with_context(|| format!("reading {}", next.display()));
                }
            };
            if kind.is_symlink() {
                hops += 1;
                if hops > MAX_HOPS {
                    return Ok(None);
                }
                let target = std::fs::read_link(&next)
                    .with_context(|| format!("reading {}", next.display()))?;
                steps.extend(reversed_steps(&target));
            } else if self.within_reach(&next) {
                real = next;
            } else {
                return Ok(None);
            }
        }
        Ok(real.starts_with(&self.root).then_some(real))
    }

    /// Inside the root, or one of the real directories above it that a path down to it passes.
    fn within_reach(&self, real: &Path) -> bool {
        real.starts_with(&self.root) || self.root.starts_with(real)
    }
}

fn reversed_steps(path: &Path) -> Vec<OsString> {
    path.components()
        .rev()
        .map(|step| step.as_os_str().to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{link, skill};

    /// A source at `<tmp>/src` holding `skill/`, and a sibling directory `<tmp>/outside` with a file.
    fn source() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("src");
        let outside = dir.path().join("outside");
        skill(&root, "skill");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("file"), "").unwrap();
        (dir, root, outside)
    }

    fn skill_escapes(root: &Path) -> bool {
        Containment::new(root)
            .unwrap()
            .escapes(&root.join("skill"))
            .unwrap()
    }

    #[test]
    fn a_link_straight_out_of_the_source_escapes() {
        let (_dir, root, outside) = source();
        link(&root, "skill/x", outside.join("file"));

        assert!(skill_escapes(&root));
    }

    #[test]
    fn a_chain_whose_last_link_leaves_the_source_escapes() {
        let (_dir, root, outside) = source();
        link(&root, "skill/a", "b");
        link(&root, "skill/b", outside.join("file"));

        assert!(skill_escapes(&root));
    }

    #[test]
    fn a_chain_that_passes_outside_and_comes_back_escapes() {
        let (_dir, root, outside) = source();
        std::fs::write(root.join("z"), "").unwrap();
        link(&outside, "y", root.join("z"));
        link(&root, "skill/a", outside.join("y"));

        assert!(skill_escapes(&root));
    }

    #[test]
    fn a_dangling_link_escapes() {
        let (_dir, root, _) = source();
        link(&root, "skill/a", "missing");

        assert!(skill_escapes(&root));
    }

    #[test]
    fn links_that_resolve_through_each_other_forever_escape_like_a_dangling_link() {
        let (_dir, root, _) = source();
        link(&root, "skill/a", "b");
        link(&root, "skill/b", "a");

        assert!(skill_escapes(&root));
    }

    #[test]
    fn a_link_to_a_directory_holding_an_escaping_link_escapes() {
        let (_dir, root, outside) = source();
        link(&root, "shared/x", outside.join("file"));
        link(&root, "skill/lib", "../shared");

        assert!(skill_escapes(&root));
    }

    #[test]
    fn links_that_stay_inside_the_source_do_not_escape() {
        let (_dir, root, _) = source();
        std::fs::create_dir_all(root.join("shared")).unwrap();
        std::fs::write(root.join("shared/file"), "").unwrap();
        link(&root, "skill/ref", "../shared/file");
        link(&root, "skill/absolute", root.join("shared"));

        assert!(!skill_escapes(&root));
    }

    #[test]
    fn a_link_loop_is_scanned_once_and_does_not_escape() {
        let (_dir, root, _) = source();
        link(&root, "skill/loop", "..");

        assert!(!skill_escapes(&root));
    }

    #[test]
    fn a_skill_directory_that_is_itself_an_escaping_link_escapes() {
        let (_dir, root, outside) = source();
        std::fs::remove_dir_all(root.join("skill")).unwrap();
        link(&root, "skill", &outside);

        assert!(skill_escapes(&root));
    }
}
