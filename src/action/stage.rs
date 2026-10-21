use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Context;

/// An edit written to a temp file beside the real file, ready to rename over it.
pub(super) struct Staged {
    target: PathBuf,
    tmp: PathBuf,
    original: String,
}

/// Temp file in the real file's directory, so a symlinked config (dotfiles) stays a symlink
/// and readers never see a half-written file. The temp is created fresh with the original
/// mode (never wider, even briefly), then given exactly the original permissions.
pub(super) fn stage(file: &Path, original: String, contents: &str) -> anyhow::Result<Staged> {
    let target = fs::canonicalize(file)?;
    let name = target
        .file_name()
        .context("no file name")?
        .to_string_lossy();
    let tmp = target.with_file_name(format!(".{name}.skillz-tmp"));
    let perms = fs::metadata(&target)?.permissions();
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        opts.mode(perms.mode());
    }
    // A leftover from a crashed run would block `create_new`.
    let _ = fs::remove_file(&tmp);
    let result = opts
        .open(&tmp)
        .and_then(|mut f| f.write_all(contents.as_bytes()))
        .and_then(|()| fs::set_permissions(&tmp, perms));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result?;
    Ok(Staged {
        target,
        tmp,
        original,
    })
}

/// Renames every temp file into place; if one fails, restores those already renamed (best
/// effort) and drops the rest.
pub(super) fn commit(staged: &[Staged]) -> anyhow::Result<()> {
    for (i, s) in staged.iter().enumerate() {
        if let Err(e) = fs::rename(&s.tmp, &s.target) {
            for done in &staged[..i] {
                let _ = fs::write(&done.target, &done.original);
            }
            discard(&staged[i..]);
            return Err(e).with_context(|| format!("write {}", s.target.display()));
        }
    }
    Ok(())
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
            stage(&a, "{}".into(), "{\"a\":1}").unwrap(),
            stage(&b, "[]".into(), "[1]").unwrap(),
        ];
        fs::remove_file(&staged[1].tmp).unwrap();
        assert!(commit(&staged).is_err());
        assert_eq!(fs::read_to_string(&a).unwrap(), "{}");
        assert_eq!(fs::read_to_string(&b).unwrap(), "[]");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
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

        let staged = stage(&file, "a = 1\n".into(), "b = 2\n").unwrap();
        let meta = fs::symlink_metadata(&staged.tmp).unwrap();
        assert!(meta.is_file());
        assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        assert_eq!(fs::read_to_string(&staged.tmp).unwrap(), "b = 2\n");
        assert_eq!(fs::read_to_string(&victim).unwrap(), "keep");
    }
}
