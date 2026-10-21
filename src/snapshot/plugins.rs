use std::collections::BTreeMap;

use super::{Accept, Collector, merge_agents, own};
use crate::model::{Agent, Augmentation, Kind};
use crate::profile::Entry;

/// Plugins whose marketplace has a source. Marketplaces themselves are not saved: a plugin
/// entry brings its own.
#[derive(Default)]
pub(super) struct Plugins(BTreeMap<(String, String, String), Vec<Agent>>);

impl Collector for Plugins {
    fn accept(&mut self, item: &Augmentation, agents: Vec<Agent>) -> Accept {
        match (item.kind, &item.source) {
            (Kind::Marketplace, _) => Accept::Taken,
            (Kind::Plugin, Some(source)) => {
                let plugin = item.name.split('@').next().unwrap_or(&item.name);
                let key = (source.clone(), item.group.clone(), plugin.to_string());
                merge_agents(self.0.entry(key).or_default(), agents);
                Accept::Taken
            }
            _ => Accept::Pass,
        }
    }

    fn finish(&mut self, default: &[Agent]) -> Vec<Entry> {
        std::mem::take(&mut self.0)
            .into_iter()
            .map(|((source, marketplace, name), agents)| Entry::Plugin {
                source,
                marketplace,
                name,
                agents: own(agents, default),
            })
            .collect()
    }
}
