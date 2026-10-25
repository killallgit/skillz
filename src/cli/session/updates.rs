//! The plan for updating the installed items a user picked.

use crate::app::apply::Switch;
use crate::cli::deps::Deps;
use crate::cli::scope::complete_installed;
use skillz_harness::env::Env;
use skillz_harness::model::Scope;
use skillz_harness::update;

/// An update step list for each picked item that can be updated; the rest drop out.
pub(super) fn update_plan(
    deps: &Deps,
    env: &Env,
    scope: Scope,
    ids: &[String],
) -> anyhow::Result<Switch> {
    let copies = complete_installed(deps, env, scope)?
        .into_iter()
        .flat_map(|item| item.parts().to_vec())
        .filter(|item| ids.contains(&item.id))
        .collect();
    let updates = crate::app::merge::merge(copies)
        .into_iter()
        .map(|item| (update::steps(&deps.adapters, &item, &env.project), item))
        .filter(|(steps, _)| !steps.is_empty())
        .map(|(steps, item)| (item, steps))
        .collect();
    Ok(Switch::updating(updates))
}
