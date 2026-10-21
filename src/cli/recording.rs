//! Remembering in the library what an install left behind, so it can be installed again later.

use super::deps::Deps;
use super::scope::installed;
use crate::env::Env;
use crate::library::{now_secs, record_installed, sync_installed};
use crate::model::{Augmentation, Scope};
use crate::store::Store;

/// Add to the library what is installed now but was not, or was different, in `before`, then
/// sync it with everything else installed in both scopes.
pub(super) fn record_since(
    deps: &Deps,
    store: &Store,
    (env, scope): (&Env, Scope),
    before: &[Augmentation],
) -> anyhow::Result<()> {
    let after = installed(deps, env, scope);
    let everything = [Scope::Project, Scope::Global]
        .map(|s| installed(deps, env, s))
        .concat();
    store.update(|document| {
        record_installed(&mut document.items, (before, &after), now_secs());
        sync_installed(&mut document.items, &everything, now_secs());
        Ok(())
    })
}
