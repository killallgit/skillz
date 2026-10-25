use std::fs;
use std::path::{Path, PathBuf};

/// The optional directories of a skill and the files in each, relative to the skill.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Layout {
    pub scripts: Vec<PathBuf>,
    pub references: Vec<PathBuf>,
    pub assets: Vec<PathBuf>,
}

/// What the skill in `dir` carries besides `SKILL.md`. A missing directory is an empty list.
pub fn layout(dir: &Path) -> Layout {
    Layout {
        scripts: files(dir, "scripts"),
        references: files(dir, "references"),
        assets: files(dir, "assets"),
    }
}

fn files(dir: &Path, sub: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    collect(&dir.join(sub), Path::new(sub), &mut found);
    found.sort();
    found
}

fn collect(path: &Path, relative: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let name = relative.join(entry.file_name());
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => collect(&entry.path(), &name, found),
            Ok(_) => found.push(name),
            Err(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_optional_directories_list_their_files_and_missing_ones_are_empty() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("scripts/sub")).unwrap();
        fs::write(dir.path().join("scripts/run.sh"), "").unwrap();
        fs::write(dir.path().join("scripts/sub/b.py"), "").unwrap();
        fs::create_dir_all(dir.path().join("references")).unwrap();
        fs::write(dir.path().join("references/REFERENCE.md"), "").unwrap();

        assert_eq!(
            layout(dir.path()),
            Layout {
                scripts: vec!["scripts/run.sh".into(), "scripts/sub/b.py".into()],
                references: vec!["references/REFERENCE.md".into()],
                assets: vec![],
            }
        );
    }
}
