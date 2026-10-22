//! The plan for updating the installed items a user picked.

use crate::apply::Switch;
use crate::cli::deps::Deps;
use crate::cli::scope::installed;
use crate::env::Env;
use crate::model::Scope;
use crate::update;

/// An update step list for each picked item that can be updated; the rest drop out.
pub(super) fn update_plan(deps: &Deps, env: &Env, scope: Scope, ids: &[String]) -> Switch {
    let updates = installed(deps, env, scope)
        .into_iter()
        .filter(|item| ids.contains(&item.id))
        .map(|item| (update::steps(&item, &env.project), item))
        .filter(|(steps, _)| !steps.is_empty())
        .map(|(steps, item)| (item, steps))
        .collect();
    Switch::updating(updates)
}
