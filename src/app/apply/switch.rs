//! What bringing a project to a profile involves. Pure data: nothing here reads or changes the
//! machine.

use crate::app::remove::Plan;
use crate::app::review::{ItemEvent, Target};
use skillz_harness::Spec;
use skillz_harness::action::Action;
use skillz_harness::model::Installed;
use skillz_harness::provision::Unsupported;

/// Build it with `Applier::plan`, show it, then `Applier::run` it.
#[derive(Debug)]
pub struct Switch {
    pub to: String,
    /// Wanted entries already in place.
    pub present: Vec<Spec>,
    /// Installed items to remove first: the ones the profile does not want, in exact mode.
    pub removals: Vec<Installed>,
    /// The ids among `removals` that no profile could install again.
    pub unsourced: Vec<String>,
    /// Entries (or the parts of them) not in place yet, each with its install steps.
    pub installs: Vec<(Spec, Vec<Action>)>,
    pub unsupported: Vec<Unsupported>,
    /// Installed items to bring to their latest version, each with the steps that do it.
    pub updates: Vec<(Installed, Vec<Action>)>,
}

impl Switch {
    /// Every item the run will work on, in the order it starts them.
    pub fn planned(&self) -> Vec<ItemEvent> {
        let removing = self
            .removals
            .iter()
            .map(|item| ItemEvent::started(Target::Remove(item.id.clone()), item.name.clone()));
        let installing = self
            .installs
            .iter()
            .filter(|(_, steps)| !steps.is_empty())
            .map(|(entry, _)| {
                ItemEvent::started(Target::Install(entry.clone()), entry.to_string())
            });
        let updating = self.updates.iter().map(|(item, _)| {
            ItemEvent::started(Target::Update(item.id.clone()), item.name.clone())
        });
        removing.chain(installing).chain(updating).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.removals.is_empty()
            && self.installs.is_empty()
            && self.unsupported.is_empty()
            && self.updates.is_empty()
    }

    /// A switch that only updates `items`.
    pub fn updating(items: Vec<(Installed, Vec<Action>)>) -> Self {
        Switch {
            to: "update".into(),
            present: vec![],
            removals: vec![],
            unsourced: vec![],
            installs: vec![],
            unsupported: vec![],
            updates: items,
        }
    }

    /// The removal and install steps in the order `run` takes them.
    pub fn steps(&self) -> Vec<Action> {
        Plan::from_owned(&self.removals)
            .steps
            .into_iter()
            .chain(self.installs.iter().flat_map(|(_, steps)| steps.clone()))
            .chain(self.updates.iter().flat_map(|(_, steps)| steps.clone()))
            .collect()
    }

    pub fn is_unsourced(&self, item: &Installed) -> bool {
        self.unsourced.contains(&item.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_switch_has_nothing_to_show_or_run() {
        let switch = Switch::updating(vec![]);

        assert!(switch.is_empty());
        assert!(switch.steps().is_empty());
    }
}
