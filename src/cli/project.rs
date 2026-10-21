use std::io::Write;
use std::path::PathBuf;

use anyhow::bail;

use super::args::ProjectCommand;
use super::deps::Deps;
use super::scope::project_env;
use super::screen::Rows;
use crate::profile;
use crate::workspace::Workspace;

pub(super) fn project_command(
    deps: &Deps,
    action: ProjectCommand,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let workspace = Workspace::in_env(&project_env(deps, &deps.cwd));
    match action {
        ProjectCommand::List => list_projects(deps, &workspace, out),
        ProjectCommand::Add { path } => add_project(deps, &workspace, path, out),
        ProjectCommand::Remove { path } => {
            if !workspace.remove(&deps.cwd.join(path))? {
                bail!("not a registered project");
            }
            Ok(())
        }
    }
}

fn list_projects(deps: &Deps, workspace: &Workspace, out: &mut dyn Write) -> anyhow::Result<()> {
    let store = profile::Store::in_env(&project_env(deps, &deps.cwd));
    for row in Rows::new(deps, &store).projects(workspace)? {
        let state = row.profile.as_deref().unwrap_or("-");
        let gone = if row.missing { "  (missing)" } else { "" };
        writeln!(out, "{state:<16} {}{gone}", row.path.display())?;
    }
    Ok(())
}

fn add_project(
    deps: &Deps,
    workspace: &Workspace,
    path: Option<PathBuf>,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let dir = deps.cwd.join(path.unwrap_or_else(|| PathBuf::from(".")));
    if workspace.add(&dir)? {
        writeln!(out, "added {}", dir.canonicalize()?.display())?;
    }
    Ok(())
}
