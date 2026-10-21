//! A batch of picked installs and removals as one plan, run item by item.

use anyhow::Context;

use crate::apply::{Applier, Each, Switch};
use crate::cli::deps::Deps;
use crate::cli::scope::installed;
use crate::env::Env;
use crate::library::Catalog;
use crate::model::Scope;
use crate::profile::{Entry, Profile};
use crate::review::Outcome;

/// What the user picked: entries to install and the ids of installed items to remove.
pub(super) struct Selection<'a> {
    pub install: &'a [Entry],
    pub remove: &'a [String],
}

/// Installs for what is missing (from the link the library recorded, when it has one),
/// removals for what is still installed. A picked item that
/// has since changed on disk simply drops out of the plan.
pub(super) fn selection_plan(
    deps: &Deps,
    env: &Env,
    applier: &Applier,
    (picked, library): (&Selection, &Catalog),
) -> anyhow::Result<Switch> {
    let mut wanted = Profile::new("selection");
    wanted.entries = picked
        .install
        .iter()
        .map(|entry| library.installable(entry))
        .collect();
    let mut switch = applier.plan(&wanted, false).context("planning installs")?;
    switch.removals = installed(deps, env, Scope::Project)
        .into_iter()
        .filter(|item| picked.remove.contains(&item.id))
        .collect();
    Ok(switch)
}

pub(super) fn outcome_of_each(switch: &Switch, each: Each) -> Outcome {
    Outcome {
        removed: switch.removals.len(),
        installed: each.installed,
        updated: each.updated,
        skipped: switch.unsupported.len(),
        still_installed: each
            .removed
            .map(|o| o.still_installed.into_iter().map(|a| a.name).collect())
            .unwrap_or_default(),
        failures: each.failures,
        ..Outcome::default()
    }
}
