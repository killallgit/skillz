//! The projects skillz knows about, kept in `<skillz dir>/workspace.toml`. Which profile each
//! one has loaded lives in the `apply::Ledger`; profiles themselves in `profile::Store`.

use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::atomic::write_atomic;
use crate::env::Env;
use crate::read_optional::read_optional;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct File {
    #[serde(default)]
    projects: Vec<PathBuf>,
}

pub struct Workspace {
    path: PathBuf,
}

impl Workspace {
    pub fn new(path: PathBuf) -> Self {
        Workspace { path }
    }

    pub fn in_env(env: &Env) -> Self {
        Workspace::new(env.skillz_dir().join("workspace.toml"))
    }

    fn read(&self) -> anyhow::Result<File> {
        match read_optional(&self.path)? {
            Some(text) => toml_edit::de::from_str(&text)
                .with_context(|| format!("invalid {}", self.path.display())),
            None => Ok(File::default()),
        }
    }

    fn write(&self, file: &File) -> anyhow::Result<()> {
        write_atomic(&self.path, &toml_edit::ser::to_string_pretty(file)?)
    }

    /// Registered projects, sorted. Ones whose directory is gone are still listed so they can
    /// be seen and removed.
    pub fn projects(&self) -> anyhow::Result<Vec<PathBuf>> {
        let mut projects = self.read()?.projects;
        projects.sort();
        Ok(projects)
    }

    /// Register an existing directory (by its real path). Returns whether it was new.
    pub fn add(&self, dir: &Path) -> anyhow::Result<bool> {
        let dir = dir
            .canonicalize()
            .with_context(|| format!("no such directory: {}", dir.display()))?;
        anyhow::ensure!(dir.is_dir(), "not a directory: {}", dir.display());
        let mut file = self.read()?;
        if file.projects.contains(&dir) {
            return Ok(false);
        }
        file.projects.push(dir);
        self.write(&file)?;
        Ok(true)
    }

    /// Forget a project, given as registered or as a path that still exists. Returns whether
    /// anything was removed.
    pub fn remove(&self, dir: &Path) -> anyhow::Result<bool> {
        let real = dir.canonicalize().ok();
        let mut file = self.read()?;
        let before = file.projects.len();
        file.projects
            .retain(|p| p != dir && Some(p) != real.as_ref());
        let removed = file.projects.len() != before;
        if removed {
            self.write(&file)?;
        }
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn ws() -> (tempfile::TempDir, Workspace) {
        let dir = tempfile::tempdir().unwrap();
        let ws = Workspace::new(dir.path().join("cfg/workspace.toml"));
        (dir, ws)
    }

    #[test]
    fn projects_are_added_once_by_real_path_and_listed_sorted() {
        let (dir, ws) = ws();
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        assert!(ws.projects().unwrap().is_empty());
        assert!(ws.add(&b).unwrap());
        assert!(ws.add(&a).unwrap());
        assert!(!ws.add(&a.join("../a")).unwrap());
        let listed = ws.projects().unwrap();
        assert_eq!(
            listed,
            [a.canonicalize().unwrap(), b.canonicalize().unwrap()]
        );
    }

    #[test]
    fn only_existing_directories_can_be_added() {
        let (dir, ws) = ws();
        assert!(ws.add(&dir.path().join("missing")).is_err());
        let file = dir.path().join("f");
        fs::write(&file, "x").unwrap();
        assert!(ws.add(&file).is_err());
    }

    #[test]
    fn a_project_can_be_forgotten_even_after_its_directory_is_gone() {
        let (dir, ws) = ws();
        let a = dir.path().join("a");
        fs::create_dir_all(&a).unwrap();
        ws.add(&a).unwrap();
        let registered = ws.projects().unwrap().remove(0);
        fs::remove_dir(&a).unwrap();
        assert_eq!(ws.projects().unwrap().len(), 1, "gone but still listed");
        assert!(ws.remove(&registered).unwrap());
        assert!(!ws.remove(&registered).unwrap());
        assert!(ws.projects().unwrap().is_empty());
    }

    #[test]
    fn removing_by_a_path_that_resolves_to_the_registered_one() {
        let (dir, ws) = ws();
        let a = dir.path().join("a");
        fs::create_dir_all(&a).unwrap();
        ws.add(&a).unwrap();
        assert!(ws.remove(&a.join("../a")).unwrap());
    }

    #[test]
    fn a_corrupt_file_is_an_error() {
        let (_dir, ws) = ws();
        fs::create_dir_all(ws.path.parent().unwrap()).unwrap();
        fs::write(&ws.path, "projects = 3").unwrap();
        assert!(ws.projects().is_err());
        assert!(ws.add(Path::new(".")).is_err());
    }
}
