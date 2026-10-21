use std::io::Write;

use anyhow::bail;

use super::deps::Deps;
use crate::env::Env;
use crate::model::Scope;
use crate::workspace::Workspace;
use crate::{apply, profile, remove, spinner};

/// Where a profile is loaded: one scope of one project's environment.
pub(super) struct Target<'a> {
    pub(super) env: &'a Env,
    pub(super) scope: Scope,
}

/// What `use` was asked to do.
pub(super) struct Switching<'a> {
    /// The profile to load; `None` unloads the current one.
    pub(super) profile: Option<&'a str>,
    pub(super) dry_run: bool,
}

/// Show what moving the target to the named profile changes, ask, then do it with one status
/// line.
pub(super) fn use_profile(
    deps: &Deps,
    target: &Target,
    switching: &Switching,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let ledger = apply::Ledger::in_env(target.env);
    let profile = switching
        .profile
        .map(|name| profile::Store::in_env(target.env).load_flat(name))
        .transpose()?;
    let applier = make_applier(deps, target, &ledger);
    let switch = applier.plan(profile.as_ref())?;

    print_switch(out, &switch, target.scope)?;
    if switch.is_empty() {
        writeln!(out, "already up to date")?;
        return Ok(());
    }
    print_steps(out, &switch)?;
    if switching.dry_run {
        return Ok(());
    }
    if !deps.prompter.confirm("Proceed?")? {
        writeln!(out, "aborted")?;
        return Ok(());
    }
    let done = execute(deps, &applier, &switch)?;
    if let Err(e) = remember_project(target) {
        writeln!(out, "warning: could not remember this project: {e:#}")?;
    }
    report(out, &switch, &done)
}

fn print_switch(out: &mut dyn Write, switch: &apply::Switch, scope: Scope) -> anyhow::Result<()> {
    let name = |p: &Option<String>| p.clone().unwrap_or_else(|| "none".into());
    writeln!(
        out,
        "{scope}: {} -> {}",
        name(&switch.from),
        name(&switch.to)
    )?;
    for l in &switch.keep {
        writeln!(out, "  keep    {}", l.entry)?;
    }
    for l in &switch.drop {
        writeln!(out, "  remove  {}", l.entry)?;
    }
    for (entry, _) in &switch.installs {
        writeln!(out, "  install {entry}")?;
    }
    for u in &switch.unsupported {
        writeln!(
            out,
            "  skip    {} for {}: not supported at {scope} scope",
            u.entry, u.agent
        )?;
    }
    Ok(())
}

fn print_steps(out: &mut dyn Write, switch: &apply::Switch) -> anyhow::Result<()> {
    writeln!(out, "Plan:")?;
    for step in switch.steps() {
        writeln!(out, "  {}", step.describe())?;
    }
    Ok(())
}

fn make_applier<'a>(
    deps: &'a Deps,
    target: &'a Target,
    ledger: &'a apply::Ledger,
) -> apply::Applier<'a> {
    apply::Applier {
        adapters: &deps.adapters,
        env: target.env,
        scope: target.scope,
        ledger,
    }
}

fn execute(
    deps: &Deps,
    applier: &apply::Applier,
    switch: &apply::Switch,
) -> anyhow::Result<apply::Done> {
    let spinner = spinner::Spinner::start("switching");
    let removal = remove::Plan::from_owned(&switch.removals);
    let result = applier.run(switch, deps.runner, &mut |step| {
        let Some(s) = &spinner else { return };
        let installing = switch
            .installs
            .iter()
            .find(|(_, steps)| steps.contains(step));
        match installing {
            Some((entry, _)) => s.set(&format!("installing {entry}")),
            None => s.set(&format!("removing {}", removal.owner(step))),
        }
    });
    drop(spinner);
    result
}

/// Remembering the project is a convenience: a failure is reported, not fatal, since the
/// switch itself already worked.
fn remember_project(target: &Target) -> anyhow::Result<()> {
    if target.scope == Scope::Project {
        Workspace::in_env(target.env).add(&target.env.project)?;
    }
    Ok(())
}

fn report(out: &mut dyn Write, switch: &apply::Switch, done: &apply::Done) -> anyhow::Result<()> {
    writeln!(
        out,
        "removed {} items, installed {} items",
        switch.removals.len(),
        done.added.len()
    )?;
    if let Some(left) = done
        .removed
        .as_ref()
        .filter(|o| !o.still_installed.is_empty())
    {
        bail!(
            "still installed after removal: {}",
            left.still_installed_names().join(", ")
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_workspace_that_cannot_be_updated_is_an_error_not_silence() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let env = Env::new(home.path().into(), project.path().into());
        std::fs::create_dir_all(env.skillz_dir()).unwrap();
        std::fs::write(env.skillz_dir().join("workspace.toml"), "projects = 3").unwrap();
        let target = Target {
            env: &env,
            scope: Scope::Project,
        };

        assert!(remember_project(&target).is_err());
    }
}
