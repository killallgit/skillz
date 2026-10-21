use std::collections::HashSet;

use crate::model::Augmentation;
use crate::profile::{Entry, Profile};
use crate::provision::{self, Unsupported};

use super::switch::{Diff, Switch, diff_entries};
use super::{Applier, Loaded};

impl Applier<'_> {
    /// What the scope needs to become `target` (`None` = nothing loaded). Pure apart from
    /// reading the ledger and the inventory.
    pub fn plan(&self, target: Option<&Profile>) -> anyhow::Result<Switch> {
        let record = self.ledger.get(&self.ledger_key())?;
        let wanted = target.map(Profile::resolved).unwrap_or_default();
        let current = record.as_ref().map(|r| r.entries.as_slice());
        let Diff { keep, drop, add } = diff_entries(current.unwrap_or_default(), &wanted);
        let removals = self.items_of(&drop)?;
        let (installs, unsupported) = self.installs_for(&add);
        Ok(Switch {
            from: record.map(|r| r.profile),
            to: target.map(|p| p.name.clone()),
            keep,
            drop,
            removals,
            installs,
            unsupported,
        })
    }

    /// The installed items the entries added.
    fn items_of(&self, entries: &[Loaded]) -> anyhow::Result<Vec<Augmentation>> {
        if entries.is_empty() {
            return Ok(vec![]);
        }
        let ids: HashSet<&str> = entries
            .iter()
            .flat_map(|l| l.items.iter().map(|o| o.id.as_str()))
            .collect();
        Ok(self
            .installed()?
            .into_iter()
            .filter(|i| ids.contains(i.id.as_str()))
            .collect())
    }

    /// The install steps for each entry, and what some agent cannot take.
    fn installs_for(
        &self,
        entries: &[Entry],
    ) -> (Vec<(Entry, Vec<crate::action::Action>)>, Vec<Unsupported>) {
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
