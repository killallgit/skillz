//! A project's `.skillz` directory: shared profiles next to local state that stays out of git.

use std::fs;
use std::path::Path;

use anyhow::Context;

const GITIGNORE: &str = "state.json\nstate.lock\nenvs/\n*.skillz-tmp\n";

/// Create `dir` with a `.gitignore` for the local state. An existing directory is left alone.
pub fn ensure(dir: &Path) -> anyhow::Result<()> {
    if dir.exists() {
        return Ok(());
    }
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    fs::write(dir.join(".gitignore"), GITIGNORE)
        .with_context(|| format!("writing {}", dir.join(".gitignore").display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_directory_gets_a_gitignore() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".skillz");

        ensure(&dir).unwrap();

        assert!(dir.join(".gitignore").is_file());
    }

    #[test]
    fn an_existing_directory_is_left_alone() {
        let tmp = tempfile::tempdir().unwrap();

        ensure(tmp.path()).unwrap();

        assert!(!tmp.path().join(".gitignore").exists());
    }
}
