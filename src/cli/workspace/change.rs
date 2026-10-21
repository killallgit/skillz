//! One change to the project from the workspace: review the plan on screen, show progress while
//! the steps run, then report how it ended. The terminal stays up throughout, since the tools
//! run with their output captured.

use std::path::Path;

use anyhow::Context;

use super::notice::Notice;
use super::plan_view::{plan_view, step_label};
use crate::apply::{self, Done, Switch};
use crate::cli::deps::Deps;
use crate::cli::scope::{installed, project_env};
use crate::env::Env;
use crate::model::Scope;
use crate::profile::{self, Entry, Profile};
use crate::remove;
use crate::review::{Next, Outcome, Verdict};

/// What the user asked to change.
pub(super) enum Change<'a> {
    /// Install what the profile has that the project lacks.
    Apply(&'a str),
    /// Install what the profile has that the project lacks and remove everything else.
    ApplyExact(&'a str),
    Install(&'a Entry),
    /// Remove the installed item with this id.
    Remove(&'a str),
}

impl Change<'_> {
    fn title(&self) -> &'static str {
        match self {
            Change::Apply(_) | Change::ApplyExact(_) => "Apply",
            Change::Install(_) => "Install",
            Change::Remove(_) => "Remove",
        }
    }

    fn done(&self, switch: &Switch) -> Notice {
        match self {
            Change::Apply(_) | Change::ApplyExact(_) => Notice::Applied(switch.to.clone()),
            Change::Install(_) => Notice::Installed(switch.to.clone()),
            Change::Remove(_) => Notice::Removed(switch.to.clone()),
        }
    }
}

/// What one change works on: the project's environment and the effects.
struct Job<'a> {
    deps: &'a Deps<'a>,
    env: Env,
}

/// Make the change once the user approves the plan. A failed run can be retried from its
/// result screen; the plan is made again each time.
pub(super) fn change(deps: &Deps, project: &Path, what: &Change) -> anyhow::Result<Option<Notice>> {
    let job = Job {
        deps,
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
        let outcome = job.apply(&applier, &switch, what.title());
        if deps.prompter.outcome(what.title(), &outcome)? != Next::Retry {
            return Ok(Some(job.notice(what, &switch, &outcome)));
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
            Change::Install(entry) => applier.plan(&profile_of(entry), false),
            Change::Remove(id) => self.removal(id),
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

    /// A switch that only removes the installed item `id`.
    fn removal(&self, id: &str) -> anyhow::Result<Switch> {
        let item = installed(self.deps, &self.env, Scope::Project)
            .into_iter()
            .find(|item| item.id == id)
            .with_context(|| format!("{id} is no longer installed"))?;
        Ok(Switch {
            to: item.name.clone(),
            present: vec![],
            removals: vec![item],
            unsourced: vec![],
            installs: vec![],
            unsupported: vec![],
        })
    }

    fn apply(&self, applier: &apply::Applier, switch: &Switch, title: &str) -> Outcome {
        let progress = self.deps.prompter.progress(title, switch.steps().len());
        let removal = remove::Plan::from_owned(&switch.removals);
        let mut log = Vec::new();
        let result = applier.run(switch, self.deps.runner, &mut |step| {
            let label = step_label(switch, &removal, step);
            progress.step(&label);
            log.push(label);
        });
        drop(progress);
        Outcome {
            log,
            ..outcome_of(switch, result)
        }
    }

    fn notice(&self, what: &Change, switch: &Switch, outcome: &Outcome) -> Notice {
        match outcome.problems().lines().next() {
            Some(problem) => Notice::Failed(problem.to_string()),
            None => what.done(switch),
        }
    }
}

/// A profile of just `entry`, named for the plan heading.
fn profile_of(entry: &Entry) -> Profile {
    let mut profile = Profile::new(&entry.to_string());
    profile.entries = vec![entry.clone()];
    profile
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
mod tests;
