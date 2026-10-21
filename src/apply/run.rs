use crate::action::{self, Action, Report};
use crate::remove::{self, Outcome, Plan, Removal};
use crate::runner::CommandRunner;

use super::Applier;
use super::switch::Switch;

/// What a switch did.
#[derive(Debug)]
pub struct Done {
    /// `None` when nothing needed removing.
    pub removed: Option<Outcome>,
    pub installed: Report,
}

impl Applier<'_> {
    /// Remove the unwanted items, then run the install steps.
    pub fn run(
        &self,
        switch: &Switch,
        runner: &dyn CommandRunner,
        on_step: &mut dyn FnMut(&Action),
    ) -> anyhow::Result<Done> {
        let removed = self.remove_unwanted(switch, runner, on_step)?;
        let steps: Vec<Action> = switch
            .installs
            .iter()
            .flat_map(|(_, steps)| steps.clone())
            .collect();
        let installed = action::execute_with(&steps, runner, on_step)?;
        Ok(Done { removed, installed })
    }

    pub(super) fn remove_unwanted(
        &self,
        switch: &Switch,
        runner: &dyn CommandRunner,
        on_step: &mut dyn FnMut(&Action),
    ) -> anyhow::Result<Option<Outcome>> {
        if switch.removals.is_empty() {
            return Ok(None);
        }
        let removal = Removal {
            adapters: self.adapters,
            env: self.env,
            scopes: &[self.scope],
            runner,
        };
        remove::run(&Plan::from_owned(&switch.removals), &removal, on_step).map(Some)
    }
}
