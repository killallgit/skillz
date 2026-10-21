use std::collections::BTreeMap;

use super::{Accept, Collector, own};
use crate::model::{Agent, Augmentation, Installer, Kind};
use crate::profile::Entry;

/// Skills `npx skills` fetched from a source, grouped by source and agents.
#[derive(Default)]
pub(super) struct Skills(BTreeMap<(String, Vec<Agent>), Vec<String>>);

impl Collector for Skills {
    fn accept(&mut self, item: &Augmentation, agents: Vec<Agent>) -> Accept {
        match (item.kind, item.installer, &item.source) {
            (Kind::Skill, Installer::Npx, Some(source)) => {
                self.0
                    .entry((source.clone(), agents))
                    .or_default()
                    .push(item.name.clone());
                Accept::Taken
            }
            _ => Accept::Pass,
        }
    }

    fn finish(&mut self, default: &[Agent]) -> Vec<Entry> {
        std::mem::take(&mut self.0)
            .into_iter()
            .map(|((source, agents), names)| Entry::Skill {
                source,
                names: sorted(names),
                agents: own(agents, default),
            })
            .collect()
    }
}

pub(super) fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names.dedup();
    names
}
