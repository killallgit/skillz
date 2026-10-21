use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::Context;

/// The file's text, or `None` when it does not exist.
pub(super) fn read_existing(file: &Path) -> anyhow::Result<Option<String>> {
    match fs::read_to_string(file) {
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        other => Ok(Some(
            other.with_context(|| format!("read {}", file.display()))?,
        )),
    }
}
