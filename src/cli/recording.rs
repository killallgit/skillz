//! Remembering in the library what an install left behind, so it can be installed again later.

use super::deps::Deps;
use super::scope::complete_installed;
use crate::env::Env;
use crate::library::{now_secs, record_installed};
use crate::model::{Installed, Scope};
use crate::stale::Disk;
use crate::store::Store;
use crate::sync::merge;

/// Add to the library what is installed now but was not, or was different, in `before`, then
/// sync it with everything else installed in both scopes.
pub(super) fn record_since(
    deps: &Deps,
    store: &Store,
    (env, scope): (&Env, Scope),
    before: &[Installed],
) -> anyhow::Result<()> {
    let scopes = [
        complete_installed(deps, env, Scope::Project)?,
        complete_installed(deps, env, Scope::Global)?,
    ];
    let after = match scope {
        Scope::Project => &scopes[0],
        Scope::Global => &scopes[1],
    };
    store.update(|document| {
        record_installed(&mut document.items, (before, after), now_secs());
        merge(document, &scopes.concat(), &Disk);
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::registry;
    use crate::cli::scope::project_env;
    use crate::install::test_support::{FakeRunner, ScriptedPrompter};

    #[test]
    fn recording_an_install_also_picks_up_what_else_is_installed_at_user_level() {
        let dir = tempfile::tempdir().unwrap();
        let settings = dir.path().join(".claude/settings.json");
        std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
        let hook = r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"fmt"}]}]}}"#;
        std::fs::write(&settings, hook).unwrap();
        let (runner, prompter) = (FakeRunner::new(|_| {}), ScriptedPrompter::new(&[], true));
        let deps = Deps::for_test(dir.path(), registry(), (&runner, &prompter));
        let env = project_env(&deps, &dir.path().join("proj"));
        let store = Store::in_env(&env);

        record_since(&deps, &store, (&env, Scope::Project), &[]).unwrap();

        // Assert the global hook, which the install did not touch, is in the library.
        assert_eq!(store.read().unwrap().items.items().len(), 1);
    }
}
