//! Writing a file so a reader, or a crash, never sees half of it.

use std::fs;
use std::path::Path;

use anyhow::Context;

/// Replace `path` with `text`: written to a temp file in the same directory, then renamed over
/// it. Creates the parent directory.
pub fn write_atomic(path: &Path, text: &str) -> anyhow::Result<()> {
    let dir = path.parent().context("path has no parent directory")?;
    fs::create_dir_all(dir)?;
    let name = path.file_name().context("path has no file name")?;
    let tmp = dir.join(format!(
        ".{}.{}.skillz-tmp",
        name.to_string_lossy(),
        std::process::id()
    ));
    let written = fs::write(&tmp, text).and_then(|()| fs::rename(&tmp, path));
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written.with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_the_content_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("sub/f.json");

        write_atomic(&file, "one").unwrap();
        write_atomic(&file, "two").unwrap();

        // only the target remains beside the new content
        assert_eq!(fs::read_to_string(&file).unwrap(), "two");
        assert_eq!(fs::read_dir(file.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn a_failed_rename_leaves_the_old_target_and_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("f");
        fs::create_dir(&target).unwrap();

        let result = write_atomic(&target, "x");

        // a directory cannot be replaced by a file; nothing stray stays behind
        assert!(result.is_err());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        assert!(target.is_dir());
    }
}
