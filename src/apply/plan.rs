use crate::action::Action;
use crate::profile::{Entry, Profile};
use crate::provision::{self, Unsupported};

use super::Applier;
use super::standing::compare;
use super::switch::Switch;

impl Applier<'_> {
    /// What the scope needs to hold `target`: what is missing gets installed and, when `exact`,
    /// what the profile does not want gets removed. Pure apart from reading the inventory.
    pub fn plan(&self, target: &Profile, exact: bool) -> anyhow::Result<Switch> {
        let standing = compare(&target.resolved(), &self.installed()?);
        let (installs, unsupported) = self.installs_for(&standing.missing);
        let extras = if exact { standing.extras } else { vec![] };
        Ok(Switch {
            to: target.name.clone(),
            present: standing.present,
            unsourced: extras
                .iter()
                .filter(|e| !e.sourced)
                .map(|e| e.item.id.clone())
                .collect(),
            removals: extras.into_iter().map(|e| e.item).collect(),
            installs,
            unsupported,
        })
    }

    /// The install steps for each entry, and what some agent cannot take.
    fn installs_for(&self, entries: &[Entry]) -> (Vec<(Entry, Vec<Action>)>, Vec<Unsupported>) {
        let mut installs = Vec::new();
        let mut unsupported = Vec::new();
        for entry in entries {
            let planned = provision::plan_entries(
                self.adapters,
                std::slice::from_ref(entry),
                self.scope,
                self.env,
            );
            unsupported.extend(planned.unsupported);
            installs.push((entry.clone(), planned.steps));
        }
        (installs, unsupported)
    }
}
