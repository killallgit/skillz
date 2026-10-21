//! What the home screen shows: the row for the current project and the rows for profiles.

use std::path::{Path, PathBuf};

use super::deps::Deps;
use super::detail;
use super::notice::Notice;
use super::scope::project_env;
use crate::apply;
use crate::model::Scope;
use crate::profile;
use crate::tui::home::{Current, Model, ProfileRow, ProjectRow};

pub(super) fn profile_rows(store: &profile::Store) -> anyhow::Result<Vec<ProfileRow>> {
    Ok(store
        .names()?
        .into_iter()
        .map(|name| profile_row(store, name))
        .collect())
}

fn profile_row(store: &profile::Store, name: String) -> ProfileRow {
    match store.load(&name) {
        Ok(p) => ProfileRow {
            detail: detail::profile(&p),
            broken: false,
            name,
        },
        Err(e) => ProfileRow {
            detail: detail::broken(&e),
            broken: true,
            name,
        },
    }
}

fn project_row(deps: &Deps, store: &profile::Store, path: PathBuf) -> anyhow::Result<ProjectRow> {
    let env = project_env(deps, &path);
    let record = apply::Ledger::in_env(&env).get(&apply::Ledger::key(&env, Scope::Project))?;
    let wanted = record
        .as_ref()
        .and_then(|r| store.load_flat(&r.profile).ok())
        .map(|p| p.entries);
    Ok(ProjectRow {
        profile: record.as_ref().map(|r| r.profile.clone()),
        short: short_path(deps, &path),
        missing: false,
        detail: detail::project(&path, false, record.as_ref(), wanted),
        path,
    })
}

pub(super) fn screen_model(
    deps: &Deps,
    store: &profile::Store,
    project: &Path,
    notice: Option<Notice>,
) -> anyhow::Result<Model> {
    let here = project_row(deps, store, project.to_path_buf())?;
    Ok(Model {
        current: Some(Current {
            short: here.short.clone(),
            profile: here.profile.clone(),
        }),
        projects: vec![here],
        profiles: profile_rows(store)?,
        undo: notice.as_ref().and_then(Notice::undo),
        notice: notice.map(|n| n.to_string()),
    })
}

fn short_path(deps: &Deps, path: &Path) -> String {
    match path.strip_prefix(&deps.home) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::test_support::{FakeRunner, ScriptedPrompter};

    fn with_deps<T>(f: impl FnOnce(&Deps, &Path) -> T) -> T {
        let dir = tempfile::tempdir().unwrap();
        let runner = FakeRunner::new(|_| {});
        let prompter = ScriptedPrompter::new(&[], true);
        let deps = Deps::for_test(dir.path(), vec![], (&runner, &prompter));
        f(&deps, dir.path())
    }

    #[test]
    fn a_project_without_a_loaded_profile_has_none_on_its_row() {
        with_deps(|deps, dir| {
            let store = profile::Store::in_env(&project_env(deps, dir));

            let here = project_row(deps, &store, dir.to_path_buf()).unwrap();

            // Assert the typed row carries no profile.
            assert_eq!(here.profile, None);
        });
    }

    #[test]
    fn an_unreadable_ledger_fails_the_row_instead_of_hiding_the_profile() {
        with_deps(|deps, dir| {
            let env = project_env(deps, dir);
            let store = profile::Store::in_env(&env);
            std::fs::create_dir_all(env.skillz_dir()).unwrap();
            std::fs::write(env.skillz_dir().join("state.json"), "{not json").unwrap();

            let row = project_row(deps, &store, dir.to_path_buf());

            // A corrupt ledger is an error, not a silently empty row.
            assert!(row.is_err());
        });
    }

    #[test]
    fn a_readable_profile_is_listed_and_not_marked_broken() {
        with_deps(|deps, dir| {
            let store = profile::Store::in_env(&project_env(deps, dir));
            store.save(&profile::Profile::new("ok")).unwrap();

            let rows = profile_rows(&store).unwrap();

            // Assert the name and the broken flag, not the rendered detail.
            assert_eq!((rows[0].name.as_str(), rows[0].broken), ("ok", false));
        });
    }
}
