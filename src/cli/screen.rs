//! What the workspace screen shows: the rows for registered projects and for profiles.

use std::path::{Path, PathBuf};

use super::deps::Deps;
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
    notice: Option<String>,
) -> anyhow::Result<Model> {
    let ledger = apply::Ledger::in_env(&project_env(deps, &deps.cwd));
    let projects = workspace
        .projects()?
        .into_iter()
        .map(|path| project_row(deps, &ledger, path))
        .collect();
    let profiles = store
        .names()?
        .into_iter()
        .map(|name| profile_row(store, name))
        .collect();
    let here = project_row(deps, &ledger, deps.cwd.clone());
    Ok(Model {
        current: Some(Current {
            short: here.short,
            profile: here.profile,
        }),
        projects,
        profiles,
        notice,
    })
}

fn project_row(deps: &Deps, ledger: &apply::Ledger, path: PathBuf) -> ProjectRow {
    let key = apply::Ledger::key(&project_env(deps, &path), Scope::Project);
    let record = ledger.get(&key).ok().flatten();
    let missing = !path.is_dir();
    ProjectRow {
        profile: record.as_ref().map(|r| r.profile.clone()),
        short: short_path(deps, &path),
        missing,
        detail: project_detail(&path, missing, record.as_ref()),
        path,
    }
}

fn project_detail(path: &Path, missing: bool, record: Option<&apply::Record>) -> Vec<String> {
    let mut detail = vec![path.display().to_string()];
    if missing {
        detail.push("directory is missing".into());
    }
    for loaded in record.iter().flat_map(|r| &r.entries) {
        detail.push(loaded.entry.to_string());
        detail.extend(loaded.items.iter().map(|o| format!("    {}", o.name)));
    }
    detail
}

fn profile_row(store: &profile::Store, name: String) -> ProfileRow {
    match store.load(&name) {
        Ok(p) => ProfileRow {
            detail: profile_detail(&p),
            broken: false,
            name,
        },
        Err(e) => ProfileRow {
            detail: vec![format!("{e:#}")],
            broken: true,
            name,
        },
    }
}

fn profile_detail(profile: &profile::Profile) -> Vec<String> {
    let extends =
        (!profile.extends.is_empty()).then(|| format!("extends {}", profile.extends.join(", ")));
    extends
        .into_iter()
        .chain(profile.entries.iter().map(ToString::to_string))
        .collect()
}

fn short_path(deps: &Deps, path: &Path) -> String {
    match path.strip_prefix(&deps.home) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}
