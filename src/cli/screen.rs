//! What the workspace screen shows: the rows for registered projects and for profiles.

use std::path::{Path, PathBuf};

use super::deps::Deps;
use super::detail;
use super::notice::Notice;
use super::scope::project_env;
use crate::apply;
use crate::model::Scope;
use crate::profile;
use crate::tui::home::{Current, Model, Places, ProfileRow, ProjectRow};
use crate::workspace::Workspace;

/// Builds the project and profile rows from one ledger and profile store.
pub(super) struct Rows<'a> {
    deps: &'a Deps<'a>,
    ledger: apply::Ledger,
    store: &'a profile::Store,
}

impl<'a> Rows<'a> {
    pub(super) fn new(deps: &'a Deps<'a>, store: &'a profile::Store) -> Self {
        Rows {
            deps,
            ledger: apply::Ledger::in_env(&project_env(deps, &deps.cwd)),
            store,
        }
    }

    pub(super) fn projects(&self, workspace: &Workspace) -> anyhow::Result<Vec<ProjectRow>> {
        workspace
            .projects()?
            .into_iter()
            .map(|path| self.project(path))
            .collect()
    }

    pub(super) fn profiles(&self) -> anyhow::Result<Vec<ProfileRow>> {
        Ok(self
            .store
            .names()?
            .into_iter()
            .map(|name| self.profile(name))
            .collect())
    }

    fn project(&self, path: PathBuf) -> anyhow::Result<ProjectRow> {
        let key = apply::Ledger::key(&project_env(self.deps, &path), Scope::Project);
        let record = self.ledger.get(&key)?;
        let missing = !path.is_dir();
        let wanted = record
            .as_ref()
            .and_then(|r| self.store.load_flat(&r.profile).ok())
            .map(|p| p.entries);
        Ok(ProjectRow {
            profile: record.as_ref().map(|r| r.profile.clone()),
            short: short_path(self.deps, &path),
            missing,
            detail: detail::project(&path, missing, record.as_ref(), wanted),
            path,
        })
    }

    fn profile(&self, name: String) -> ProfileRow {
        match self.store.load(&name) {
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
}

pub(super) fn screen_model(
    deps: &Deps,
    store: &profile::Store,
    workspace: &Workspace,
    notice: Option<Notice>,
) -> anyhow::Result<Model> {
    let rows = Rows::new(deps, store);
    let here = rows.project(deps.cwd.clone())?;
    Ok(Model {
        places: Places {
            start: deps.cwd.clone(),
            home: deps.home.clone(),
        },
        current: Some(Current {
            short: here.short,
            profile: here.profile,
        }),
        projects: rows.projects(workspace)?,
        profiles: rows.profiles()?,
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
    fn a_project_row_records_its_profile_and_whether_the_directory_is_gone() {
        with_deps(|deps, dir| {
            let store = profile::Store::in_env(&project_env(deps, dir));
            let rows = Rows::new(deps, &store);

            let here = rows.project(dir.to_path_buf()).unwrap();
            let gone = rows.project(dir.join("gone")).unwrap();

            // Assert the typed rows carry no profile and the missing flag.
            assert_eq!(here.profile, None);
            assert!(!here.missing);
            assert!(gone.missing);
        });
    }

    #[test]
    fn an_unreadable_ledger_fails_the_rows_instead_of_hiding_the_profile() {
        with_deps(|deps, dir| {
            let env = project_env(deps, dir);
            let store = profile::Store::in_env(&env);
            std::fs::create_dir_all(env.skillz_dir()).unwrap();
            std::fs::write(env.skillz_dir().join("applied.json"), "{not json").unwrap();

            let row = Rows::new(deps, &store).project(dir.to_path_buf());

            // A corrupt ledger is an error, not a silently empty row.
            assert!(row.is_err());
        });
    }

    #[test]
    fn a_readable_profile_is_listed_and_not_marked_broken() {
        with_deps(|deps, dir| {
            let store = profile::Store::in_env(&project_env(deps, dir));
            store.save(&profile::Profile::new("ok")).unwrap();

            let rows = Rows::new(deps, &store).profiles().unwrap();

            // Assert the name and the broken flag, not the rendered detail.
            assert_eq!((rows[0].name.as_str(), rows[0].broken), ("ok", false));
        });
    }
}
