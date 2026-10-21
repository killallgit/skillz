//! Switching a project's profile from the workspace: review the plan on screen, show progress
//! while the steps run, then report how it ended. The terminal stays up throughout, since the
//! tools run with their output captured.

use std::path::{Path, PathBuf};

use super::deps::Deps;
use super::notice::Notice;
use super::plan_view::{plan_view, step_label};
use super::scope::project_env;
use crate::apply::{self, Done, Switch};
use crate::env::Env;
use crate::model::Scope;
use crate::profile;
use crate::remove;
use crate::review::{Next, Outcome, Verdict};

/// What one switch works on: the project's environment, ledger and profile store.
struct Job<'a> {
    deps: &'a Deps<'a>,
    env: Env,
    ledger: apply::Ledger,
    store: profile::Store,
    project: PathBuf,
}

/// Load `profile` into `project` (or unload with `None`) once the user approves the plan. A
/// failed run can be retried from its result screen; the plan is made again each time.
pub(super) fn switch(
    deps: &Deps,
    project: &Path,
    profile: Option<&str>,
) -> anyhow::Result<Option<Notice>> {
    let env = project_env(deps, project);
    let job = Job {
        deps,
        ledger: apply::Ledger::in_env(&env),
        store: profile::Store::in_env(&env),
        env,
        project: project.to_path_buf(),
    };
    loop {
        let applier = job.applier();
        let wanted = profile.map(|n| job.store.load_flat(n)).transpose()?;
        let switch = applier.plan(wanted.as_ref())?;
        if switch.is_empty() {
            return Ok(Some(Notice::UpToDate));
        }
        match deps.prompter.plan(&plan_view(&switch, Scope::Project))? {
            Verdict::Cancel => return Ok(None),
            Verdict::DryRun => return Ok(Some(Notice::DryRun(switch.steps().len()))),
            Verdict::Apply => {}
        }
        let outcome = job.apply(&applier, &switch);
        if deps.prompter.outcome("Switch", &outcome)? != Next::Retry {
            return Ok(Some(job.notice(&switch, &outcome)));
        }
    }
}

impl Job<'_> {
    fn applier(&self) -> apply::Applier<'_> {
        apply::Applier {
            adapters: &self.deps.adapters,
            env: &self.env,
            scope: Scope::Project,
            ledger: &self.ledger,
        }
    }

    fn apply(&self, applier: &apply::Applier, switch: &Switch) -> Outcome {
        let progress = self
            .deps
            .prompter
            .progress("Switching", switch.steps().len());
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

    fn notice(&self, switch: &Switch, outcome: &Outcome) -> Notice {
        match outcome.problems().lines().next() {
            Some(problem) => Notice::Failed(problem.to_string()),
            None => Notice::Switched {
                project: self.project.clone(),
                from: switch.from.clone(),
                to: switch.to.clone(),
            },
        }
    }
}

fn outcome_of(switch: &Switch, result: anyhow::Result<Done>) -> Outcome {
    let skipped = switch.unsupported.len();
    match result {
        Ok(done) => Outcome {
            removed: switch.removals.len(),
            installed: done.added.len(),
            skipped,
            still_installed: done.removed.map(|o| o.still_installed).unwrap_or_default(),
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
mod tests {
    use super::*;

    fn empty_switch() -> Switch {
        Switch {
            from: None,
            to: None,
            keep: vec![],
            drop: vec![],
            removals: vec![],
            installs: vec![],
            unsupported: vec![],
        }
    }

    #[test]
    fn a_failed_run_is_a_failure_and_a_clean_one_is_not() {
        let failed = outcome_of(&empty_switch(), Err(anyhow::anyhow!("boom")));
        let clean = outcome_of(
            &empty_switch(),
            Ok(Done {
                removed: None,
                installed: Default::default(),
                added: vec![],
            }),
        );

        // Only the error run reports a problem.
        assert_eq!((failed.is_clean(), clean.is_clean()), (false, true));
    }
}
