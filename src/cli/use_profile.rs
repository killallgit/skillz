use std::io::Write;

use anyhow::bail;

use super::deps::Deps;
use crate::env::Env;
use crate::model::Scope;
use crate::{apply, profile, remove, spinner};

/// Where a profile is loaded: one scope of one project's environment.
pub(super) struct Target<'a> {
    pub(super) env: &'a Env,
    pub(super) scope: Scope,
}

pub(super) struct Switching<'a> {
    pub(super) profile: &'a str,
    /// Also remove what the profile does not want.
    pub(super) exact: bool,
    pub(super) dry_run: bool,
}

/// Show what bringing the target to the named profile changes, ask, then do it with one status
/// line.
pub(super) fn use_profile(
    deps: &Deps,
    target: &Target,
    switching: &Switching,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let profile = profile::Store::in_env(target.env).load_flat(switching.profile)?;
    let applier = make_applier(deps, target);
    let switch = applier.plan(&profile, switching.exact)?;

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
    report(out, &switch, &done)
}

fn print_switch(out: &mut dyn Write, switch: &apply::Switch, scope: Scope) -> anyhow::Result<()> {
    writeln!(out, "{scope}: {}", switch.to)?;
    for entry in &switch.present {
        writeln!(out, "  keep    {entry}")?;
    }
    for item in &switch.removals {
        let unsourced = if switch.is_unsourced(item) {
            "  (no known source: cannot be reinstalled)"
        } else {
            ""
        };
        writeln!(out, "  remove  {}{unsourced}", item.label(true))?;
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

fn make_applier<'a>(deps: &'a Deps, target: &'a Target) -> apply::Applier<'a> {
    apply::Applier {
        adapters: &deps.adapters,
        env: target.env,
        scope: target.scope,
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

fn report(out: &mut dyn Write, switch: &apply::Switch, done: &apply::Done) -> anyhow::Result<()> {
    writeln!(
        out,
        "removed {} items, installed {} entries",
        switch.removals.len(),
        switch.installs.len()
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
