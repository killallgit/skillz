use crate::profile::Profile;
use skillz_harness::Spec;
use skillz_harness::action::Action;
use skillz_harness::provision::{self, Unsupported};

use super::Applier;
use super::standing::compare;
use super::switch::Switch;

impl Applier<'_> {
    /// What the scope needs to hold `target`: what is missing gets installed and, when `exact`,
    /// what the profile does not want gets removed. Reads source locations and the installed inventory.
    pub fn plan(&self, target: &Profile, exact: bool) -> anyhow::Result<Switch> {
        let target = target.clone().in_env(self.env);
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
            updates: vec![],
        })
    }

    /// The install steps for each entry, and what some agent cannot take.
    fn installs_for(&self, entries: &[Spec]) -> (Vec<(Spec, Vec<Action>)>, Vec<Unsupported>) {
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
            if !planned.steps.is_empty() {
                installs.push((entry.clone(), planned.steps));
            }
        }
        (installs, unsupported)
    }
}
