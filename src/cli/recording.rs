//! Remembering in the library what an install left behind, so it can be installed again later.

use super::deps::Deps;
use super::scope::installed;
use crate::env::Env;
use crate::library::record_installed;
use crate::model::{Augmentation, Scope};
use crate::store::Store;

pub(super) fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Add to the library what is installed now but was not, or was different, in `before`.
pub(super) fn record_since(
    deps: &Deps,
    store: &Store,
    (env, scope): (&Env, Scope),
    before: &[Augmentation],
) -> anyhow::Result<()> {
    let after = installed(deps, env, scope);
    store.update(|document| {
        record_installed(&mut document.items, (before, &after), now_secs());
        Ok(())
    })
}
