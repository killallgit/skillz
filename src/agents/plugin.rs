use std::path::Path;

use crate::read_optional::read_dir;

pub(super) fn skill_names(root: &Path) -> anyhow::Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in read_dir(&root.join("skills"))? {
        match std::fs::metadata(entry.path().join("SKILL.md")) {
            Ok(metadata) if metadata.is_file() => {
                if let Some(name) = entry.file_name().to_str() {
                    names.push(name.to_string());
                }
            }
            Ok(_) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) => {}
            Err(error) => return Err(error.into()),
        }
    }
    names.sort();
    Ok(names)
}
