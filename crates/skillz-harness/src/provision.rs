//! Turn a profile into the steps that install it. Pure: nothing runs here; `action::execute`
//! does. Skills go through `npx skills` for every agent at once; anything else is asked of each
//! agent's adapter (`Harness::plan_install`).

use crate::Spec;
use crate::action::Action;
use crate::agents::{Harness, adapter_for};
use crate::env::Env;
use crate::model::{Agent, Scope};
use crate::npx::commands;

/// An entry one agent has no way to take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsupported {
    pub entry: Spec,
    pub agent: Agent,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Provision {
    pub steps: Vec<Action>,
    pub unsupported: Vec<Unsupported>,
}

/// The steps that install `entries`, which already name the agents they are for.
pub fn plan_entries(
    adapters: &[Box<dyn Harness>],
    entries: &[Spec],
    scope: Scope,
    env: &Env,
) -> Provision {
    let mut out = Provision::default();
    for entry in entries {
        match skills_step(adapters, entry, scope, env) {
            Some(step) => out.steps.push(step),
            None => {
                let per_agent = provision_per_agent(adapters, entry, scope, env);
                out.steps.extend(per_agent.steps);
                out.unsupported.extend(per_agent.unsupported);
            }
        }
    }
    out
}

/// One `npx skills add` for all the entry's agents that this build knows; `None` unless the
/// entry is a skill.
fn skills_step(
    adapters: &[Box<dyn Harness>],
    entry: &Spec,
    scope: Scope,
    env: &Env,
) -> Option<Action> {
    let Spec::Skill { source, names, .. } = entry else {
        return None;
    };
    let known: Vec<Agent> = entry
        .agents()
        .iter()
        .copied()
        .filter(|a| adapters.iter().any(|ad| ad.agent() == *a))
        .collect();
    let (program, args) = commands::add_command(source, names, &known, scope);
    Some(Action::required(&program, args, &env.project))
}

/// Ask each of the entry's agents for its own steps; the ones that cannot are reported.
fn provision_per_agent(
    adapters: &[Box<dyn Harness>],
    entry: &Spec,
    scope: Scope,
    env: &Env,
) -> Provision {
    let mut out = Provision::default();
    for &agent in entry.agents() {
        let steps = adapter_for(adapters, agent).and_then(|a| a.plan_install(entry, scope, env));
        match steps {
            Some(steps) => out.steps.extend(steps),
            None => out.unsupported.push(Unsupported {
                entry: entry.clone(),
                agent,
            }),
        }
    }
    out
}
