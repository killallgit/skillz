use crate::action::{self, Action};
use crate::model::Augmentation;
use crate::remove::Outcome;
use crate::review::{ItemEvent, Target};
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

/// Who hears about the run as it goes: every step, and every item starting and ending.
pub struct Watch<'a> {
    pub step: &'a mut dyn FnMut(&Action),
    pub item: &'a mut dyn FnMut(ItemEvent),
}

impl Applier<'_> {
    /// Like `run`, but a failing entry does not stop the ones after it. Steps an earlier entry
    /// already ran are not run again.
    pub fn run_each(&self, switch: &Switch, runner: &dyn CommandRunner, watch: &mut Watch) -> Each {
        let mut failures = Vec::new();
        let removed = self.remove_each(switch, runner, watch, &mut failures);
        let mut ran: Vec<&Action> = Vec::new();
        let mut installed = 0;
        for (entry, steps) in &switch.installs {
            let started = ItemEvent::started(Target::Install(entry.clone()), entry.to_string());
            (watch.item)(started.clone());
            let fresh: Vec<Action> = steps.iter().filter(|s| !ran.contains(s)).cloned().collect();
            let result = action::execute_with(&fresh, runner, &mut *watch.step);
            ran.extend(steps);
            installed += usize::from(result.is_ok());
            (watch.item)(ended(&started, result.map(drop), &mut failures));
        }
        let updated = self.update_each(switch, runner, watch, &mut failures);
        Each {
            removed,
            installed,
            updated,
            failures,
        }
    }

    fn remove_each(
        &self,
        switch: &Switch,
        runner: &dyn CommandRunner,
        watch: &mut Watch,
        failures: &mut Vec<String>,
    ) -> Option<Outcome> {
        let started: Vec<ItemEvent> = switch.removals.iter().map(removing).collect();
        started.iter().for_each(|event| (watch.item)(event.clone()));
        let (outcome, error) = match self.remove_unwanted(switch, runner, &mut *watch.step) {
            Ok(outcome) => (outcome, None),
            Err(e) => (None, Some(format!("{e:#}"))),
        };
        if let Some(error) = &error {
            failures.push(format!("removing: {error}"));
        }
        for (item, event) in switch.removals.iter().zip(&started) {
            let left = outcome
                .as_ref()
                .is_some_and(|o| o.still_installed.iter().any(|s| s.id == item.id));
            let result = match (&error, left) {
                (Some(error), _) => Err(error.clone()),
                (None, true) => Err("still installed after removal".to_string()),
                (None, false) => Ok(()),
            };
            (watch.item)(event.ended(result));
        }
        outcome
    }

    fn update_each(
        &self,
        switch: &Switch,
        runner: &dyn CommandRunner,
        watch: &mut Watch,
        failures: &mut Vec<String>,
    ) -> usize {
        let mut updated = 0;
        for (item, steps) in &switch.updates {
            let started = ItemEvent::started(Target::Update(item.id.clone()), item.name.clone());
            (watch.item)(started.clone());
            let result = action::execute_with(steps, runner, &mut *watch.step);
            updated += usize::from(result.is_ok());
            (watch.item)(ended(&started, result.map(drop), failures));
        }
        updated
    }
}

fn removing(item: &Augmentation) -> ItemEvent {
    ItemEvent::started(Target::Remove(item.id.clone()), item.name.clone())
}

/// `started` ended with `result`; a failure also goes on the list under the item's name.
fn ended(started: &ItemEvent, result: anyhow::Result<()>, failures: &mut Vec<String>) -> ItemEvent {
    let result = result.map_err(|e| format!("{e:#}"));
    if let Err(reason) = &result {
        failures.push(format!("{}: {reason}", started.label));
    }
    started.ended(result)
}
