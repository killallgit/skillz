//! A batch of picked installs and removals as one plan, run item by item.

use anyhow::Context;

use crate::apply::{Applier, Report, Switch};
use crate::cli::deps::Deps;
use crate::cli::scope::complete_installed;
use crate::harness::Spec;
use crate::harness::env::Env;
use crate::harness::model::{Agent, Installed};
use crate::library::Catalog;
use crate::merge::merge;
use crate::profile::Profile;
use crate::review::Outcome;

/// What the user picked: entries to install and the ids of installed items to remove.
#[derive(Clone, Copy)]
pub(in crate::cli) struct Selection<'a> {
    pub install: &'a [Spec],
    pub remove: &'a [String],
    /// The agents to remove from; every agent when `None`.
    pub only: Option<&'a [Agent]>,
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
    switch.removals = complete_installed(deps, env, applier.scope)?
        .into_iter()
        .filter(|item| picked.remove.contains(&item.id))
        .filter_map(|item| match picked.only {
            Some(agents) => only_for(item, agents),
            None => Some(item),
        })
        .collect();
    Ok(switch)
}

/// The copies of `item` that belong wholly to `agents`, folded back into one item; `None` when
/// no copy does.
fn only_for(item: Installed, agents: &[Agent]) -> Option<Installed> {
    let kept: Vec<Installed> = item
        .parts()
        .iter()
        // A copy shared by several agents is one set of files, so it goes only when all of them do.
        .filter(|part| part.agents.iter().all(|agent| agents.contains(agent)))
        .cloned()
        .collect();
    match kept.len() == item.parts().len() {
        true => Some(item),
        false => merge(kept).into_iter().next(),
    }
}

pub(super) fn outcome_of(switch: &Switch, each: Report) -> Outcome {
    Outcome {
        removed: each.removed.as_ref().map_or(0, |removed| {
            switch
                .removals
                .len()
                .saturating_sub(removed.still_installed.len())
        }),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::model::{Installer, Kind, Scope};

    fn subagent(agent: Agent) -> Installed {
        Installed {
            details: Default::default(),
            id: format!("subagent:{agent}:reviewer"),
            name: "reviewer".into(),
            kind: Kind::Subagent,
            agents: vec![agent],
            scope: Scope::Project,
            group: "local".into(),
            source: None,
            installer: Installer::Local,
            origin: Default::default(),
            children: vec![],
            hooks: vec![],
            members: vec![],
            removal: vec![],
        }
    }

    fn both() -> Installed {
        merge(vec![subagent(Agent::Claude), subagent(Agent::Codex)]).remove(0)
    }

    #[test]
    fn removal_narrowed_to_one_agent_keeps_only_that_agents_copy() {
        let narrowed = only_for(both(), &[Agent::Codex]).unwrap();

        assert_eq!(narrowed.agents, [Agent::Codex]);
        assert_eq!(narrowed.id, "subagent:codex:reviewer");
    }

    #[test]
    fn a_copy_shared_by_two_agents_stays_unless_both_are_chosen() {
        let shared = Installed {
            agents: vec![Agent::Claude, Agent::Codex],
            ..subagent(Agent::Claude)
        };

        assert!(only_for(shared.clone(), &[Agent::Claude]).is_none());
        assert!(only_for(shared, &[Agent::Claude, Agent::Codex]).is_some());
    }

    #[test]
    fn every_agent_leaves_the_item_whole_and_none_drops_it() {
        let whole = only_for(both(), &[Agent::Claude, Agent::Codex]).unwrap();

        assert_eq!(whole.agents, [Agent::Claude, Agent::Codex]);
        assert!(only_for(both(), &[]).is_none());
    }
}
