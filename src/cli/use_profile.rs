use std::io::Write;

use anyhow::bail;

use super::deps::Deps;
use crate::env::Env;
use crate::model::{Augmentation, Scope};
use crate::workspace::Workspace;
use crate::{apply, profile, remove, spinner};

/// `use`: show what moving `scope` to the named profile (`None` = no profile) changes, ask, then
/// do it with one status line.
pub(super) fn use_profile(
    deps: &Deps,
    env: &Env,
    scope: Scope,
    name: Option<&str>,
    dry_run: bool,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let ledger = apply::Ledger::in_env(env);
    let target = name
        .map(|n| profile::Store::in_env(env).load(n))
        .transpose()?;
    let switch = apply::plan(&deps.adapters, target.as_ref(), scope, env, &ledger)?;

    let label = |p: &Option<String>| p.clone().unwrap_or_else(|| "none".into());
    writeln!(
        out,
        "{scope}: {} -> {}",
        label(&switch.from),
        label(&switch.to)
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
    if switch.is_empty() {
        writeln!(out, "already up to date")?;
        return Ok(());
    }
    writeln!(out, "Plan:")?;
    for step in switch.steps() {
        writeln!(out, "  {}", step.describe())?;
    }
    if dry_run {
        return Ok(());
    }
    if !deps.prompter.confirm("Proceed?")? {
        writeln!(out, "aborted")?;
        return Ok(());
    }

    let spinner = spinner::Spinner::start("switching");
    let refs: Vec<&Augmentation> = switch.removals.iter().collect();
    let removal = remove::Plan::new(&refs, &[]);
    let result = apply::run(
        &deps.adapters,
        &switch,
        scope,
        env,
        &ledger,
        deps.runner,
        &mut |step| {
            let Some(s) = &spinner else { return };
            let installing = switch
                .installs
                .iter()
                .find(|(_, steps)| steps.contains(step));
            match installing {
                Some((entry, _)) => s.set(&format!("installing {entry}")),
                None => s.set(&format!("removing {}", removal.owner(step))),
            }
        },
    );
    drop(spinner);
    let done = result?;
    if scope == Scope::Project {
        // Remembering the project is a convenience; the switch itself already worked.
        let _ = Workspace::in_env(env).add(&env.project);
    }
    writeln!(
        out,
        "removed {} items, installed {} items",
        switch.removals.len(),
        done.added.len()
    )?;
    if let Some(left) = done.removed.filter(|o| !o.still_installed.is_empty()) {
        bail!(
            "still installed after removal: {}",
            left.still_installed.join(", ")
        );
    }
    Ok(())
}
