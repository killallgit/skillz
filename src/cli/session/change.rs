//! One change to the project from the workspace: review the plan on screen, show progress while
//! the steps run, then report how it ended. The terminal stays up throughout, since the tools
//! run with their output captured.

use super::Session;
use super::notice::Notice;
use super::plan_view::{plan_view, step_label};
use super::selection::{Selection, outcome_of, selection_plan};
use super::updates::update_plan;
use crate::action::Action;
use crate::apply::{self, Switch, Watch};
use crate::cli::deps::Deps;
use crate::cli::recording::record_since;
use crate::cli::scope::{complete_installed, project_env};
use crate::env::Env;
use crate::model::Scope;
use crate::profile;
use crate::remove;
use crate::review::{ItemEvent, Next, Outcome, Phase, Target, Verdict};
use crate::stale::Disk;
use crate::store::Store;

/// What the user asked to change.
pub(in crate::cli) enum Change<'a> {
    /// Install what the profile has that the project lacks.
    Apply(&'a str),
    /// Install what the profile has that the project lacks and remove everything else.
    ApplyExact(&'a str),
    /// Install and remove the items the user picked, one at a time.
    Selection(&'a Selection<'a>),
    /// Update the installed items with these ids, one at a time.
    Update(&'a [String]),
}

impl Change<'_> {
    fn title(&self) -> &'static str {
        match self {
            Change::Apply(_) | Change::ApplyExact(_) => "Apply",
            Change::Selection(_) => "Change",
            Change::Update(_) => "Update",
        }
    }

    fn done(&self, switch: &Switch, outcome: &Outcome) -> Notice {
        match self {
            Change::Apply(_) | Change::ApplyExact(_) => Notice::Applied(switch.to.clone()),
            Change::Selection(_) => Notice::Changed {
                installed: outcome.installed,
                removed: outcome.removed,
            },
            Change::Update(_) => Notice::ItemsUpdated(outcome.updated),
        }
    }
}

/// What one change works on: the scope of the project's environment and the effects.
struct Job<'a> {
    deps: &'a Deps<'a>,
    store: &'a Store,
    env: Env,
    scope: Scope,
}

/// Make the change once the user approves the plan. A failed run can be retried from its
/// result screen; the plan is made again each time.
pub(in crate::cli) fn change(
    session: &Session,
    scope: Scope,
    what: &Change,
) -> anyhow::Result<Option<Notice>> {
    let (deps, store) = (session.deps, &session.store);
    let job = Job {
        deps,
        store,
        env: project_env(deps, &session.project),
        scope,
    };
    let mut completed_updates = std::collections::HashSet::new();
    loop {
        let applier = job.applier();
        let mut switch = job.plan(&applier, what)?;
        switch
            .updates
            .retain(|(item, _)| !completed_updates.contains(&item.id));
        if switch.is_empty() {
            return Ok(Some(Notice::UpToDate));
        }
        match deps.prompter.plan(&plan_view(&switch, job.scope))? {
            Verdict::Cancel => return Ok(None),
            Verdict::DryRun => return Ok(Some(Notice::DryRun(switch.steps().len()))),
            Verdict::Apply => {}
        }
        let before = complete_installed(deps, &job.env, job.scope)?;
        let outcome = job.apply(&applier, &switch, what);
        for event in &outcome.items {
            if let (Target::Update(id), Phase::Succeeded) = (&event.target, &event.phase) {
                completed_updates.insert(id.clone());
            }
        }
        let recorded = record_since(deps, store, (&job.env, job.scope), &before);
        if deps.prompter.outcome(what.title(), &outcome)? != Next::Retry {
            return Ok(Some(job.notice(what, &switch, &outcome, recorded)));
        }
    }
}

impl Job<'_> {
    fn applier(&self) -> apply::Applier<'_> {
        apply::Applier {
            adapters: &self.deps.adapters,
            env: &self.env,
            scope: self.scope,
            probe: &Disk,
        }
    }

    fn plan(&self, applier: &apply::Applier, what: &Change) -> anyhow::Result<Switch> {
        match what {
            Change::Apply(name) => self.apply_plan(applier, name, false),
            Change::ApplyExact(name) => self.apply_plan(applier, name, true),
            Change::Selection(picked) => {
                let library = self.store.read()?.items.in_env(&self.env);
                selection_plan(self.deps, &self.env, applier, (picked, &library))
            }
            Change::Update(ids) => update_plan(self.deps, &self.env, self.scope, ids),
        }
    }

    fn apply_plan(
        &self,
        applier: &apply::Applier,
        name: &str,
        exact: bool,
    ) -> anyhow::Result<Switch> {
        let wanted = profile::Store::in_env(&self.env).load_flat(name)?;
        applier.plan(&wanted, exact)
    }

    fn apply(&self, applier: &apply::Applier, switch: &Switch, what: &Change) -> Outcome {
        let progress = self.deps.prompter.live(what.title(), switch.steps().len());
        let removal = remove::Plan::from_owned(&switch.removals);
        let mut log = Vec::new();
        let mut items = Vec::new();
        let mut on_step = |step: &Action| {
            let label = step_label(switch, &removal, step);
            progress.step(&label);
            log.push(label);
        };
        let mut on_item = |event: ItemEvent| {
            progress.item(&event);
            if event.phase != Phase::Started {
                items.push(event);
            }
        };
        let mut watch = Watch {
            step: &mut on_step,
            item: &mut on_item,
        };
        let outcome = outcome_of(switch, applier.run(switch, self.deps.runner, &mut watch));
        drop(progress);
        Outcome {
            log,
            items,
            ..outcome
        }
    }

    fn notice(
        &self,
        what: &Change,
        switch: &Switch,
        outcome: &Outcome,
        recorded: anyhow::Result<()>,
    ) -> Notice {
        if !outcome.is_clean() {
            return Notice::Failed(outcome.problems());
        }
        match recorded {
            Err(e) => Notice::Failed(format!("installed, but not recorded: {e:#}")),
            Ok(()) => what.done(switch, outcome),
        }
    }
}

#[cfg(test)]
mod provenance_tests;
#[cfg(test)]
mod scope_tests;
#[cfg(test)]
mod selection_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod update_tests;
