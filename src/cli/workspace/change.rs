//! One change to the project from the workspace: review the plan on screen, show progress while
//! the steps run, then report how it ended. The terminal stays up throughout, since the tools
//! run with their output captured.

use std::path::Path;

use super::notice::Notice;
use super::plan_view::{plan_view, step_label};
use super::selection::{Selection, outcome_of_each, selection_plan};
use super::updates::update_plan;
use crate::action::Action;
use crate::apply::{self, Done, Switch, Watch};
use crate::cli::deps::Deps;
use crate::cli::recording::record_since;
use crate::cli::scope::{installed, project_env};
use crate::env::Env;
use crate::model::Scope;
use crate::profile;
use crate::remove;
use crate::review::{ItemEvent, Next, Outcome, Phase, Verdict};
use crate::store::Store;

/// What the user asked to change.
pub(super) enum Change<'a> {
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

    fn done(&self, switch: &Switch) -> Notice {
        match self {
            Change::Apply(_) | Change::ApplyExact(_) => Notice::Applied(switch.to.clone()),
            Change::Selection(_) => Notice::Changed {
                installed: switch.installs.len(),
                removed: switch.removals.len(),
            },
            Change::Update(_) => Notice::ItemsUpdated(switch.updates.len()),
        }
    }
}

/// What one change works on: the project's environment and the effects.
struct Job<'a> {
    deps: &'a Deps<'a>,
    store: &'a Store,
    env: Env,
}

/// Make the change once the user approves the plan. A failed run can be retried from its
/// result screen; the plan is made again each time.
pub(super) fn change(
    deps: &Deps,
    store: &Store,
    project: &Path,
    what: &Change,
) -> anyhow::Result<Option<Notice>> {
    let job = Job {
        deps,
        store,
        env: project_env(deps, project),
    };
    loop {
        let applier = job.applier();
        let switch = job.plan(&applier, what)?;
        if switch.is_empty() {
            return Ok(Some(Notice::UpToDate));
        }
        match deps.prompter.plan(&plan_view(&switch, Scope::Project))? {
            Verdict::Cancel => return Ok(None),
            Verdict::DryRun => return Ok(Some(Notice::DryRun(switch.steps().len()))),
            Verdict::Apply => {}
        }
        let before = installed(deps, &job.env, Scope::Project);
        let outcome = job.apply(&applier, &switch, what);
        let recorded = record_since(deps, store, (&job.env, Scope::Project), &before);
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
            scope: Scope::Project,
        }
    }

    fn plan(&self, applier: &apply::Applier, what: &Change) -> anyhow::Result<Switch> {
        match what {
            Change::Apply(name) => self.apply_plan(applier, name, false),
            Change::ApplyExact(name) => self.apply_plan(applier, name, true),
            Change::Selection(picked) => {
                let library = self.store.read()?.items;
                selection_plan(self.deps, &self.env, applier, (picked, &library))
            }
            Change::Update(ids) => Ok(update_plan(self.deps, &self.env, ids)),
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
        let outcome = match what {
            Change::Selection(_) | Change::Update(_) => {
                let mut watch = Watch {
                    step: &mut on_step,
                    item: &mut on_item,
                };
                outcome_of_each(
                    switch,
                    applier.run_each(switch, self.deps.runner, &mut watch),
                )
            }
            _ => outcome_of(switch, applier.run(switch, self.deps.runner, &mut on_step)),
        };
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
        match (outcome.problems().lines().next(), recorded) {
            (Some(problem), _) => Notice::Failed(problem.to_string()),
            (None, Err(e)) => Notice::Failed(format!("installed, but not recorded: {e:#}")),
            (None, Ok(())) => what.done(switch),
        }
    }
}

fn outcome_of(switch: &Switch, result: anyhow::Result<Done>) -> Outcome {
    let skipped = switch.unsupported.len();
    match result {
        Ok(done) => Outcome {
            removed: switch.removals.len(),
            installed: switch.installs.len(),
            skipped,
            still_installed: done
                .removed
                .map(|o| o.still_installed.into_iter().map(|a| a.name).collect())
                .unwrap_or_default(),
            ..Outcome::default()
        },
        Err(e) => Outcome {
            skipped,
            failures: vec![format!("{e:#}")],
            ..Outcome::default()
        },
    }
}

#[cfg(test)]
mod provenance_tests;
#[cfg(test)]
mod selection_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod update_tests;
