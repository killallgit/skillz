use std::fs::{self, File};
use std::path::Path;

use anyhow::Context;

/// An exclusive advisory lock on a sidecar file, held until dropped. The store file itself is
/// replaced by rename, so it cannot carry the lock.
pub(super) struct Lock {
    _file: File,
}

impl Lock {
    pub(super) fn acquire(path: &Path) -> anyhow::Result<Lock> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let file = File::options()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("opening {}", path.display()))?;
        file.lock()
            .with_context(|| format!("locking {}", path.display()))?;
        Ok(Lock { _file: file })
    }
}
