use std::io::Write;

use anyhow::{Context, bail};

use super::deps::Deps;
use super::recording::record_since;
use super::scope::installed;
use crate::env::Env;
use crate::model::Scope;
use crate::store::Store;
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
    let store = Store::in_env(target.env);
    let before = installed(deps, target.env, target.scope);
    let done = execute(deps, &applier, &switch);
    let recorded = record_since(deps, &store, (target.env, target.scope), &before);
    let done = done?;
    recorded.context("installed, but the library was not updated")?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::registry;
    use crate::install::test_support::{FakeNpx, LOCKED_LINK, ScriptedPrompter};
    use crate::profile::{Entry, Profile};

    #[test]
    fn using_a_profile_records_what_it_installed_in_the_library() {
        let dir = tempfile::tempdir().unwrap();
        let env = Env::new(dir.path().into(), dir.path().into());
        let mut profile = Profile::new("web");
        profile.agents = vec![crate::model::Agent::Claude];
        profile.entries = vec![Entry::Skill {
            source: "o/r".into(),
            names: vec!["tdd".into()],
            agents: vec![],
        }];
        profile::Store::in_env(&env).save(&profile).unwrap();
        let (runner, prompter) = (FakeNpx::new(), ScriptedPrompter::new(&[], true));
        let deps = Deps::for_test(dir.path(), registry(), (&runner, &prompter));
        let target = Target {
            env: &env,
            scope: Scope::Project,
        };
        let switching = Switching {
            profile: "web",
            exact: false,
            dry_run: false,
        };

        use_profile(&deps, &target, &switching, &mut Vec::new()).unwrap();

        // Assert the library holds the skill with the link npx locked.
        let items = Store::in_env(&env).read().unwrap().items;
        let provenance = items.items()[0].provenance.clone().unwrap();
        assert_eq!(provenance.link.as_deref(), Some(LOCKED_LINK));
    }
}
