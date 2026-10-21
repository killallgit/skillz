use std::io::Write;
use std::path::PathBuf;

use super::args::{Command, InstallArgs, ListArgs, PruneArgs, ResetArgs, UninstallArgs, UseArgs};
use super::deps::Deps;
use super::list::{ListView, list};
use super::profile::profile_command;
use super::project::project_command;
use super::prune::prune;
use super::removal::{reset, uninstall};
use super::scope::{Scene, resolve, single_scope, split_target};
use super::use_profile::{Switching, Target, use_profile};
use crate::agents;
use crate::install::{self, InstallRequest, SkillSelection};

pub fn run(command: Command, deps: &Deps, out: &mut dyn Write) -> anyhow::Result<()> {
    match command {
        Command::List(args) => run_list(deps, args, out),
        Command::Uninstall(args) => run_uninstall(deps, args, out),
        Command::Prune(args) => run_prune(deps, args, out),
        Command::Reset(args) => run_reset(deps, args, out),
        Command::Use(args) => run_use(deps, args, out),
        Command::Install(args) => run_install(deps, args),
        Command::Profile { action } => profile_command(deps, action, out),
        Command::Project { action } => project_command(deps, action, out),
    }
}

fn run_list(deps: &Deps, args: ListArgs, out: &mut dyn Write) -> anyhow::Result<()> {
    let (path, query) = split_target(args.project, args.target);
    let (env, scopes) = resolve(deps, path.as_deref(), args.filter.global)?;
    let view = ListView {
        query: query.as_deref(),
        all: args.all,
        json: args.json,
        interactive: args.interactive,
    };
    list(&Scene::new(deps, &env, &scopes, &args.filter), view, out)
}

fn run_uninstall(deps: &Deps, args: UninstallArgs, out: &mut dyn Write) -> anyhow::Result<()> {
    let (env, scopes) = resolve(deps, args.project.as_deref(), args.filter.global)?;
    let scene = Scene::new(deps, &env, &scopes, &args.filter);
    uninstall(&scene, args.query.as_deref(), args.yes, args.dry_run, out)
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
    let (env, scopes) = resolve(deps, args.project.as_deref(), args.global)?;
    let target = Target {
        env: &env,
        scope: single_scope(&scopes, args.global),
    };
    let switching = Switching {
        profile: args.profile.as_deref(),
        dry_run: args.dry_run,
    };
    use_profile(deps, &target, &switching, out)
}

fn run_install(deps: &Deps, args: InstallArgs) -> anyhow::Result<()> {
    let (env, scopes) = resolve(deps, args.project.as_deref(), args.global)?;
    let request = InstallRequest {
        source: args.source,
        cwd: deps.cwd.clone(),
        agents: if args.agents.is_empty() {
            agents::all()
        } else {
            args.agents
        },
        scope: single_scope(&scopes, args.global),
        selection: skill_selection(args.all, args.skill),
    };
    install::install(&deps.adapters, &request, &env, deps.runner, deps.prompter)
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
