use std::io::Write;
use std::path::PathBuf;

use anyhow::bail;

use super::args::ProjectCommand;
use super::deps::Deps;
use super::scope::project_env;
use crate::apply;
use crate::model::Scope;
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

/// Each registered project with the profile it has loaded.
fn list_projects(deps: &Deps, workspace: &Workspace, out: &mut dyn Write) -> anyhow::Result<()> {
    let ledger = apply::Ledger::in_env(&project_env(deps, &deps.cwd));
    for dir in workspace.projects()? {
        let key = apply::Ledger::key(&project_env(deps, &dir), Scope::Project);
        let state = ledger.get(&key)?.map_or("-".to_string(), |r| r.profile);
        let gone = if dir.is_dir() { "" } else { "  (missing)" };
        writeln!(out, "{state:<16} {}{gone}", dir.display())?;
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
