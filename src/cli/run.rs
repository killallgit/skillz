use std::io::Write;
use std::path::PathBuf;

use super::args::{
    Command, Filter, InstallArgs, ListArgs, PruneArgs, ResetArgs, SyncArgs, UninstallArgs,
    UpdateArgs, UseArgs,
};
use super::deps::Deps;
use super::isolate::run_isolated;
use super::library::{available, library_command};
use super::list::{ListView, list};
use super::profile::profile_command;
use super::prune::prune;
use super::removal::{UninstallRequest, reset, uninstall};
use super::scope::{Scene, resolve, resolve_project, split_target};
use super::session::{Change, Session, with_session};
use super::sync::{SyncRequest, sync};
use super::update::{UpdateRequest, check, update};
use crate::app::install::{InstallRequest, SkillSelection};
use skillz_harness::model::Scope;

/// Runs the command and returns the process exit code: the child's for `run`, else 0.
pub fn run(command: Command, deps: &Deps, out: &mut dyn Write) -> anyhow::Result<i32> {
    match command {
        Command::Run(args) => run_isolated(deps, args, out),
        Command::List(args) => succeeded(run_list(deps, args, out)),
        Command::Uninstall(args) => succeeded(run_uninstall(deps, args, out)),
        Command::Update(args) => succeeded(run_update(deps, args, out)),
        Command::Prune(args) => succeeded(run_prune(deps, args, out)),
        Command::Reset(args) => succeeded(run_reset(deps, args, out)),
        Command::Use(args) => succeeded(run_use(deps, args, out)),
        Command::Sync(args) => succeeded(run_sync(deps, args, out)),
        Command::Install(args) => succeeded(run_install(deps, args)),
        Command::Library { action, project } => {
            succeeded(library_command(deps, action, project, out))
        }
        Command::Profile { action, project } => {
            succeeded(profile_command(deps, action, project, out))
        }
    }
}

fn succeeded(result: anyhow::Result<()>) -> anyhow::Result<i32> {
    result.map(|()| 0)
}

fn run_list(deps: &Deps, args: ListArgs, out: &mut dyn Write) -> anyhow::Result<()> {
    if args.available {
        let scope = scope_of(args.filter.global);
        let env = resolve_project(deps, args.project.as_deref())?;
        return available(deps, &env, (scope, args.json), out);
    }
    let (path, query) = split_target(args.project, args.target);
    let (env, scopes) = resolve(deps, path.as_deref(), args.filter.global)?;
    let view = ListView {
        query: query.as_deref(),
        all: args.all,
        json: args.json,
        check: args.check,
        interactive: args.interactive,
    };
    list(&Scene::new(deps, &env, &scopes, &args.filter), view, out)
}

fn run_uninstall(deps: &Deps, args: UninstallArgs, out: &mut dyn Write) -> anyhow::Result<()> {
    let (env, scopes) = resolve(deps, args.project.as_deref(), args.filter.global)?;
    let scene = Scene::new(deps, &env, &scopes, &args.filter);
    let request = UninstallRequest {
        queries: &args.queries,
        yes: args.yes,
        dry_run: args.dry_run,
    };
    uninstall(&scene, &request, out)
}

/// Acts on the one project: `-p`, or a path as the target, or the current directory; with `-g`
/// on the user's installs instead.
fn run_update(deps: &Deps, args: UpdateArgs, out: &mut dyn Write) -> anyhow::Result<()> {
    let (path, query) = split_target(args.project, args.target);
    let env = resolve_project(deps, path.as_deref())?;
    let scope = scope_of(args.filter.global);
    let scopes = [scope];
    let scene = Scene::new(deps, &env, &scopes, &args.filter);
    if args.check {
        return check(&scene, query.as_deref(), args.json, out);
    }
    let request = UpdateRequest {
        query: query.as_deref(),
        yes: args.yes,
        dry_run: args.dry_run,
        scope,
    };
    update(&scene, &request, out)
}

fn run_prune(deps: &Deps, args: PruneArgs, out: &mut dyn Write) -> anyhow::Result<()> {
    let (env, scopes) = resolve(deps, args.project.as_deref(), args.filter.global)?;
    let scene = Scene::new(deps, &env, &scopes, &args.filter);
    prune(&scene, args.dry_run, out)
}

/// Destructive, so the project you are in is the default; `-g` alone is global.
fn run_reset(deps: &Deps, args: ResetArgs, out: &mut dyn Write) -> anyhow::Result<()> {
    let path = args
        .project
        .or_else(|| (!args.filter.global).then(|| PathBuf::from(".")));
    let (env, scopes) = resolve(deps, path.as_deref(), args.filter.global)?;
    let scene = Scene::new(deps, &env, &scopes, &args.filter);
    reset(&scene, &args.keep, args.dry_run, out)
}

fn run_use(deps: &Deps, args: UseArgs, out: &mut dyn Write) -> anyhow::Result<()> {
    let env = resolve_project(deps, args.project.as_deref())?;
    let what = if args.exact {
        Change::ApplyExact(&args.profile)
    } else {
        Change::Apply(&args.profile)
    };
    let scope = match args.global {
        true => Scope::Global,
        false => Scope::Project,
    };
    with_session(deps, &env, out, args.dry_run, |session, console| {
        console.conclude(session.apply(scope, &args.profile, &what)?)
    })
}

fn run_sync(deps: &Deps, args: SyncArgs, out: &mut dyn Write) -> anyhow::Result<()> {
    let (env, scopes) = resolve(deps, args.project.as_deref(), args.global)?;
    let filter = Filter::default();
    let scene = Scene::new(deps, &env, &scopes, &filter);
    let request = SyncRequest {
        dry_run: args.dry_run,
        json: args.json,
    };
    sync(&scene, &request, out)
}

fn run_install(deps: &Deps, args: InstallArgs) -> anyhow::Result<()> {
    let env = resolve_project(deps, args.project.as_deref())?;
    let scope = scope_of(args.global);
    let request = InstallRequest {
        source: args.source,
        cwd: deps.cwd.clone(),
        agents: if args.agents.is_empty() {
            deps.adapters.iter().map(|a| a.agent()).collect()
        } else {
            args.agents
        },
        scope,
        selection: skill_selection(args.all, args.skill),
    };
    Session::open(deps, &env)
        .install_source(&request)
        .map(|_| ())
}

fn scope_of(global: bool) -> Scope {
    if global {
        Scope::Global
    } else {
        Scope::Project
    }
}

fn skill_selection(all: bool, skills: Vec<String>) -> SkillSelection {
    match (all, skills.is_empty()) {
        (true, _) => SkillSelection::All,
        (false, false) => SkillSelection::Patterns(skills),
        (false, true) => SkillSelection::Interactive,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_skill_flags_pick_how_skills_are_chosen() {
        assert_eq!(skill_selection(true, vec![]), SkillSelection::All);
        assert_eq!(
            skill_selection(false, vec!["tdd".into()]),
            SkillSelection::Patterns(vec!["tdd".into()])
        );
        assert_eq!(skill_selection(false, vec![]), SkillSelection::Interactive);
    }
}
