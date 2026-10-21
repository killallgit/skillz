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

/// A registered project and the profile it has loaded.
#[derive(Debug, PartialEq, Eq)]
struct ProjectRow {
    profile: Option<String>,
    dir: PathBuf,
    missing: bool,
}

fn project_row(dir: PathBuf, profile: Option<String>) -> ProjectRow {
    ProjectRow {
        profile,
        missing: !dir.is_dir(),
        dir,
    }
}

fn project_rows(deps: &Deps, workspace: &Workspace) -> anyhow::Result<Vec<ProjectRow>> {
    let ledger = apply::Ledger::in_env(&project_env(deps, &deps.cwd));
    workspace
        .projects()?
        .into_iter()
        .map(|dir| {
            let key = apply::Ledger::key(&project_env(deps, &dir), Scope::Project);
            let profile = ledger.get(&key)?.map(|r| r.profile);
            Ok(project_row(dir, profile))
        })
        .collect()
}

fn render_projects(rows: &[ProjectRow], out: &mut dyn Write) -> anyhow::Result<()> {
    for row in rows {
        let state = row.profile.as_deref().unwrap_or("-");
        let gone = if row.missing { "  (missing)" } else { "" };
        writeln!(out, "{state:<16} {}{gone}", row.dir.display())?;
    }
    Ok(())
}

fn list_projects(deps: &Deps, workspace: &Workspace, out: &mut dyn Write) -> anyhow::Result<()> {
    render_projects(&project_rows(deps, workspace)?, out)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_project_row_records_its_profile_and_whether_the_directory_is_gone() {
        let here = tempfile::tempdir().unwrap();

        let loaded = project_row(here.path().to_path_buf(), Some("web".into()));
        let gone = project_row(here.path().join("gone"), None);

        // Assert the typed rows carry the profile and the missing flag.
        assert_eq!(loaded.profile.as_deref(), Some("web"));
        assert!(!loaded.missing);
        assert_eq!(gone.profile, None);
        assert!(gone.missing);
    }
}
