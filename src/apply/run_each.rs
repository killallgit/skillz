use crate::action::{self, Action};
use crate::remove::Outcome;
use crate::runner::CommandRunner;

use super::Applier;
use super::switch::Switch;

/// What a switch run item by item did.
#[derive(Debug)]
pub struct Each {
    pub removed: Option<Outcome>,
    /// How many entries were installed.
    pub installed: usize,
    /// How many items were updated.
    pub updated: usize,
    /// One line per item that failed; the other items were still attempted.
    pub failures: Vec<String>,
}

impl Applier<'_> {
    /// Like `run`, but a failing entry does not stop the ones after it. Steps an earlier entry
    /// already ran are not run again.
    pub fn run_each(
        &self,
        switch: &Switch,
        runner: &dyn CommandRunner,
        on_step: &mut dyn FnMut(&Action),
    ) -> Each {
        let mut failures = Vec::new();
        let removed = self.remove_unwanted(switch, runner, on_step);
        let removed = removed
            .map_err(|e| failures.push(format!("removing: {e:#}")))
            .ok()
            .flatten();
        let mut ran: Vec<&Action> = Vec::new();
        let mut installed = 0;
        for (entry, steps) in &switch.installs {
            let fresh: Vec<Action> = steps.iter().filter(|s| !ran.contains(s)).cloned().collect();
            match action::execute_with(&fresh, runner, on_step) {
                Ok(_) => installed += 1,
                Err(e) => failures.push(format!("{entry}: {e:#}")),
            }
            ran.extend(steps);
        }
        let updated = self.update_each(switch, runner, on_step, &mut failures);
        Each {
            removed,
            installed,
            updated,
            failures,
        }
    }

    fn update_each(
        &self,
        switch: &Switch,
        runner: &dyn CommandRunner,
        on_step: &mut dyn FnMut(&Action),
        failures: &mut Vec<String>,
    ) -> usize {
        let mut updated = 0;
        for (item, steps) in &switch.updates {
            match action::execute_with(steps, runner, on_step) {
                Ok(_) => updated += 1,
                Err(e) => failures.push(format!("{}: {e:#}", item.name)),
            }
        }
        updated
    }
}
