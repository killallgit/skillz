//! Reading a file that may legitimately not exist.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::Context;

/// The file's text, or `None` when it does not exist. Any other failure is an error.
pub fn read_optional(path: &Path) -> anyhow::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

pub fn read_dir(path: &Path) -> anyhow::Result<Vec<fs::DirEntry>> {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    entries
        .collect::<Result<_, _>>()
        .with_context(|| format!("reading {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_is_none_and_an_existing_one_is_its_text() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        let missing = read_optional(&file).unwrap();
        fs::write(&file, "hi").unwrap();
        let present = read_optional(&file).unwrap();
        // None only for NotFound, Some(text) otherwise
        assert_eq!((missing, present), (None, Some("hi".to_string())));
    }

    #[test]
    fn a_directory_in_place_of_a_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        // only NotFound is swallowed
        assert!(read_optional(dir.path()).is_err());
    }
}
