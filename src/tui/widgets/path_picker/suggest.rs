//! The directories that complete a half-typed path.

use std::fs;
use std::path::{Path, PathBuf};

/// Names of the directories that start with what follows the last `/` of `typed`, inside what
/// precedes it. Hidden ones appear only when the typed prefix starts with a dot.
pub fn complete(typed: &str, home: &Path) -> Vec<String> {
    let Some((parent, prefix)) = typed.rsplit_once('/') else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(expand_home(parent, home)) else {
        return Vec::new();
    };
    let prefix = prefix.to_lowercase();
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| is_offered(name, &prefix))
        .collect();
    names.sort_by_key(|name| name.to_lowercase());
    names
}

fn is_offered(name: &str, prefix: &str) -> bool {
    let hidden = name.starts_with('.');
    (!hidden || prefix.starts_with('.')) && name.to_lowercase().starts_with(prefix)
}

/// `typed` with its last component replaced by `name`, as a directory to keep typing into.
pub fn descend(typed: &str, name: &str) -> String {
    let parent = typed.rsplit_once('/').map_or("", |(parent, _)| parent);
    format!("{parent}/{name}/")
}

fn expand_home(parent: &str, home: &Path) -> PathBuf {
    match parent.strip_prefix('~') {
        Some(rest) => PathBuf::from(format!("{}{rest}/", home.display())),
        None => PathBuf::from(format!("{parent}/")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for name in ["alpha", "Apple", "beta", ".hidden"] {
            fs::create_dir(dir.path().join(name)).unwrap();
        }
        fs::write(dir.path().join("afile"), "").unwrap();
        dir
    }

    fn typed(dir: &tempfile::TempDir, rest: &str) -> String {
        format!("{}/{rest}", dir.path().display())
    }

    #[test]
    fn an_empty_prefix_lists_every_visible_directory() {
        // Assemble
        let dir = tree();

        // Act
        let names = complete(&typed(&dir, ""), dir.path());

        // Assert: sorted, without files or hidden directories
        assert_eq!(names, ["alpha", "Apple", "beta"]);
    }

    #[test]
    fn a_prefix_narrows_the_list_ignoring_case() {
        let dir = tree();

        let names = complete(&typed(&dir, "a"), dir.path());

        assert_eq!(names, ["alpha", "Apple"]);
    }

    #[test]
    fn a_dot_prefix_offers_hidden_directories() {
        let dir = tree();

        let names = complete(&typed(&dir, "."), dir.path());

        assert_eq!(names, [".hidden"]);
    }

    #[test]
    fn a_missing_directory_offers_nothing() {
        let dir = tree();

        assert!(complete(&typed(&dir, "nope/x"), dir.path()).is_empty());
    }

    #[test]
    fn a_tilde_names_the_home_directory() {
        let dir = tree();

        let names = complete("~/b", dir.path());

        assert_eq!(names, ["beta"]);
    }

    #[test]
    fn descending_replaces_the_last_component() {
        assert_eq!(descend("/a/b/par", "partial"), "/a/b/partial/");
        assert_eq!(descend("/par", "partial"), "/partial/");
    }
}
