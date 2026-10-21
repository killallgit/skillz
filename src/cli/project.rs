use std::io::Write;
use std::path::PathBuf;

use anyhow::bail;

use super::args::ProjectCommand;
use super::deps::Deps;
use crate::apply;
use crate::env::Env;
use crate::model::Scope;
use crate::workspace::Workspace;

pub(super) fn project_command(
    deps: &Deps,
    action: ProjectCommand,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let home_env = Env {
        claude_config: deps.claude_config.clone(),
        codex_home: deps.codex_home.clone(),
        ..Env::new(deps.home.clone(), deps.cwd.clone())
    };
    let workspace = Workspace::in_env(&home_env);
    match action {
        ProjectCommand::List => {
            let ledger = apply::Ledger::in_env(&home_env);
            for dir in workspace.projects()? {
                let env = Env {
                    project: dir.clone(),
                    ..home_env.clone()
                };
                let loaded = ledger.get(&apply::Ledger::key(&env, Scope::Project))?;
                let state = loaded.map_or("-".to_string(), |r| r.profile);
                let gone = if dir.is_dir() { "" } else { "  (missing)" };
                writeln!(out, "{state:<16} {}{gone}", dir.display())?;
            }
        }
        ProjectCommand::Add { path } => {
            let dir = deps.cwd.join(path.unwrap_or_else(|| PathBuf::from(".")));
            if workspace.add(&dir)? {
                writeln!(out, "added {}", dir.canonicalize()?.display())?;
            }
        }
        ProjectCommand::Remove { path } => {
            if !workspace.remove(&deps.cwd.join(path))? {
                bail!("not a registered project");
            }
        }
    }
    Ok(())
}
