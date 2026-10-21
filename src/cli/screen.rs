//! What the workspace screen shows: the rows for registered projects and for profiles.

use std::path::{Path, PathBuf};

use super::deps::Deps;
use super::detail;
use super::notice::Notice;
use super::scope::project_env;
use crate::apply;
use crate::model::Scope;
use crate::profile;
use crate::tui::home::{Current, Model, ProfileRow, ProjectRow};
use crate::workspace::Workspace;

pub(super) fn screen_model(
    deps: &Deps,
    store: &profile::Store,
    workspace: &Workspace,
    notice: Option<Notice>,
) -> anyhow::Result<Model> {
    let ledger = apply::Ledger::in_env(&project_env(deps, &deps.cwd));
    let projects = workspace
        .projects()?
        .into_iter()
        .map(|path| project_row(deps, (&ledger, store), path))
        .collect();
    let profiles = store
        .names()?
        .into_iter()
        .map(|name| profile_row(store, name))
        .collect();
    let here = project_row(deps, (&ledger, store), deps.cwd.clone());
    Ok(Model {
        current: Some(Current {
            short: here.short,
            profile: here.profile,
        }),
        projects,
        profiles,
        undo: notice.as_ref().and_then(Notice::undo),
        notice: notice.map(|n| n.to_string()),
    })
}

fn project_row(
    deps: &Deps,
    (ledger, store): (&apply::Ledger, &profile::Store),
    path: PathBuf,
) -> ProjectRow {
    let key = apply::Ledger::key(&project_env(deps, &path), Scope::Project);
    let record = ledger.get(&key).ok().flatten();
    let missing = !path.is_dir();
    let wanted = record
        .as_ref()
        .and_then(|r| store.load_flat(&r.profile).ok())
        .map(|p| p.entries);
    ProjectRow {
        profile: record.as_ref().map(|r| r.profile.clone()),
        short: short_path(deps, &path),
        missing,
        detail: detail::project(&path, missing, record.as_ref(), wanted),
        path,
    }
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

fn short_path(deps: &Deps, path: &Path) -> String {
    match path.strip_prefix(&deps.home) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}
