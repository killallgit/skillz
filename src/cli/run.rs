use std::io::Write;
use std::path::PathBuf;

use super::args::Command;
use super::deps::Deps;
use super::list::{ListView, list};
use super::profile::profile_command;
use super::project::project_command;
use super::prune::prune;
use super::removal::{reset, uninstall};
use super::scope::{Scene, resolve, single_scope, split_target};
use super::use_profile::use_profile;
use crate::agents;
use crate::install::{self, InstallRequest, SkillSelection};
use crate::model::Scope;

pub fn run(command: Command, deps: &Deps, out: &mut dyn Write) -> anyhow::Result<()> {
    match command {
        Command::List {
            target,
            project,
            all,
            json,
            filter,
            interactive,
        } => {
            let (path, query) = split_target(project, target);
            let (env, scopes) = resolve(deps, path.as_deref(), filter.global)?;
            let view = ListView {
                query: query.as_deref(),
                all,
                json,
                interactive,
            };
            list(deps, &env, &scopes, &filter, view, out)
        }
        Command::Uninstall {
            query,
            project,
            filter,
            yes,
            dry_run,
        } => {
            let (env, scopes) = resolve(deps, project.as_deref(), filter.global)?;
            let scene = Scene {
                deps,
                env: &env,
                scopes: &scopes,
                filter: &filter,
            };
            uninstall(&scene, query.as_deref(), yes, dry_run, out)
        }
        Command::Prune {
            project,
            filter,
            dry_run,
            force: _,
        } => {
            let (env, scopes) = resolve(deps, project.as_deref(), filter.global)?;
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?;
            let scene = Scene {
                deps,
                env: &env,
                scopes: &scopes,
                filter: &filter,
            };
            prune(&scene, now.as_millis() as u64, dry_run, out)
        }
        Command::Reset {
            project,
            filter,
            keep,
            dry_run,
            force: _,
        } => {
            // Destructive, so the project you are in is the default; `-g` alone is global.
            let path = project.or_else(|| (!filter.global).then(|| PathBuf::from(".")));
            let (env, scopes) = resolve(deps, path.as_deref(), filter.global)?;
            let scene = Scene {
                deps,
                env: &env,
                scopes: &scopes,
                filter: &filter,
            };
            reset(&scene, &keep, dry_run, out)
        }
        Command::Use {
            profile,
            none: _,
            global,
            project,
            yes: _,
            dry_run,
        } => {
            let (env, scopes) = resolve(deps, project.as_deref(), global)?;
            let scope = single_scope(&scopes, global);
            use_profile(deps, &env, scope, profile.as_deref(), dry_run, out)
        }
        Command::Profile { action } => profile_command(deps, action, out),
        Command::Project { action } => project_command(deps, action, out),
        Command::Install {
            source,
            global,
            agents,
            all,
            skill,
            project,
            yes: _,
        } => {
            let (env, scopes) = resolve(deps, project.as_deref(), global)?;
            let req = InstallRequest {
                source,
                cwd: deps.cwd.clone(),
                agents: if agents.is_empty() {
                    agents::all()
                } else {
                    agents
                },
                // In the home dir (no project scope left) the project is the global one.
                scope: if global || !scopes.contains(&Scope::Project) {
                    Scope::Global
                } else {
                    Scope::Project
                },
                selection: match (all, skill.is_empty()) {
                    (true, _) => SkillSelection::All,
                    (false, false) => SkillSelection::Patterns(skill),
                    (false, true) => SkillSelection::Interactive,
                },
            };
            install::install(&deps.adapters, &req, &env, deps.runner, deps.prompter)
        }
    }
}
