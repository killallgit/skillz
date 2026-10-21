use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::read_optional::read_optional;
use anyhow::Context;

/// An edit written to a temp file beside the real file, ready to rename over it.
pub(super) struct Staged {
    target: PathBuf,
    tmp: PathBuf,
    /// The text the plan was made from; `None` when the file did not exist.
    original: Option<String>,
    planned: String,
}

/// Temp file in the real file's directory, so a symlinked config (dotfiles) stays a symlink
/// and readers never see a half-written file. The temp is created fresh with the original
/// mode (never wider, even briefly), then given exactly the original permissions. A file that
/// does not exist yet (`original` is `None`) is created along with its directory.
pub(super) fn stage(
    file: &Path,
    original: Option<String>,
    contents: &str,
) -> anyhow::Result<Staged> {
    let target = resolve(file, original.is_none())?;
    let name = target
        .file_name()
        .context("no file name")?
        .to_string_lossy();
    let tmp = target.with_file_name(format!(".{name}.skillz-tmp"));
    let perms = match original {
        Some(_) => Some(fs::metadata(&target)?.permissions()),
        None => None,
    };
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    if let Some(perms) = &perms {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        opts.mode(perms.mode());
    }
    // A leftover from a crashed run would block `create_new`.
    let _ = fs::remove_file(&tmp);
    let result = opts
        .open(&tmp)
        .and_then(|mut f| f.write_all(contents.as_bytes()))
        .and_then(|()| perms.map_or(Ok(()), |p| fs::set_permissions(&tmp, p)));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result?;
    Ok(Staged {
        target,
        tmp,
        original,
        planned: contents.to_string(),
    })
}

/// The real path of `file`; a missing one gets its directory made and is named inside it.
fn resolve(file: &Path, create: bool) -> anyhow::Result<PathBuf> {
    if !create {
        return Ok(fs::canonicalize(file)?);
    }
    let name = file.file_name().context("no file name")?;
    let dir = file
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    Ok(fs::canonicalize(dir)?.join(name))
}

/// Renames every temp file into place, but only when no target changed since the plan was
/// made. If a rename fails, undoes those already renamed (leaving any that someone has
/// changed since) and drops the rest.
pub(super) fn commit(staged: &[Staged]) -> anyhow::Result<()> {
    if let Err(e) = ensure_unchanged(staged) {
        discard(staged);
        return Err(e);
    }
    for (i, s) in staged.iter().enumerate() {
        if let Err(e) = fs::rename(&s.tmp, &s.target) {
            staged[..i].iter().for_each(rollback);
            discard(&staged[i..]);
            return Err(e).with_context(|| format!("write {}", s.target.display()));
        }
    }
    Ok(())
}

fn ensure_unchanged(staged: &[Staged]) -> anyhow::Result<()> {
    for s in staged {
        anyhow::ensure!(
            read_optional(&s.target)? == s.original,
            "{} changed while skillz was planning; nothing was written",
            s.target.display()
        );
    }
    Ok(())
}

/// Put a committed file back as it was, unless it no longer holds what was committed.
fn rollback(s: &Staged) {
    if read_optional(&s.target).ok().flatten().as_deref() != Some(&s.planned) {
        return;
    }
    let _ = match &s.original {
        Some(original) => fs::write(&s.target, original),
        None => fs::remove_file(&s.target),
    };
}

pub(super) fn discard(staged: &[Staged]) {
    for s in staged {
        let _ = fs::remove_file(&s.tmp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::testutil::*;

    #[test]
    fn commit_failure_restores_renamed_files() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a.json"), dir.path().join("b.json"));
        write(&a, "{}");
        write(&b, "[]");
        let staged = [
            stage(&a, Some("{}".into()), "{\"a\":1}").unwrap(),
            stage(&b, Some("[]".into()), "[1]").unwrap(),
        ];
        fs::remove_file(&staged[1].tmp).unwrap();
        assert!(commit(&staged).is_err());
        assert_eq!(fs::read_to_string(&a).unwrap(), "{}");
        assert_eq!(fs::read_to_string(&b).unwrap(), "[]");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn a_target_changed_since_planning_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("claude.json");
        write(&file, "{}");
        let staged = [stage(&file, Some("{}".into()), "{\"mine\":1}").unwrap()];
        write(&file, "{\"theirs\":1}");

        let result = commit(&staged);

        // the concurrent writer's content wins and no temp file is left
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&file).unwrap(), "{\"theirs\":1}");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn rollback_leaves_content_written_after_the_commit() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.json");
        write(&file, "{}");
        let staged = [stage(&file, Some("{}".into()), "{\"a\":1}").unwrap()];
        commit(&staged).unwrap();
        write(&file, "{\"newer\":1}");

        rollback(&staged[0]);

        // content that is no longer what was committed stays
        assert_eq!(fs::read_to_string(&file).unwrap(), "{\"newer\":1}");
    }

    #[test]
    fn a_missing_file_is_created_with_its_directory_and_removed_on_rollback() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("new/dir/settings.json");

        let staged = [stage(&file, None, "{}\n").unwrap()];
        commit(&staged).unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "{}\n");

        rollback(&staged[0]);
        assert!(!file.exists());
    }

    #[cfg(unix)]
    #[test]
    fn stage_creates_fresh_temp_with_original_mode() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        let victim = dir.path().join("victim");
        write(&file, "a = 1\n");
        write(&victim, "keep");
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
        // A stale temp left as a link must be replaced, not written through.
        std::os::unix::fs::symlink(&victim, dir.path().join(".config.toml.skillz-tmp")).unwrap();

        let staged = stage(&file, Some("a = 1\n".into()), "b = 2\n").unwrap();
        let meta = fs::symlink_metadata(&staged.tmp).unwrap();
        assert!(meta.is_file());
        assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        assert_eq!(fs::read_to_string(&staged.tmp).unwrap(), "b = 2\n");
        assert_eq!(fs::read_to_string(&victim).unwrap(), "keep");
    }
}
