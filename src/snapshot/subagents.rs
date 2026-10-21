use std::collections::BTreeMap;

use super::skills::sorted;
use super::{Accept, Collector, own};
use crate::model::{Agent, Augmentation, Kind};
use crate::profile::Entry;

/// Subagents copied from a source, saved by file stem.
#[derive(Default)]
pub(super) struct Subagents(BTreeMap<(String, Vec<Agent>), Vec<String>>);

impl Collector for Subagents {
    fn accept(&mut self, item: &Augmentation, agents: Vec<Agent>) -> Accept {
        match (item.kind, &item.source) {
            (Kind::Subagent, Some(source)) => {
                self.0
                    .entry((source.clone(), agents))
                    .or_default()
                    .push(file_stem(item));
                Accept::Taken
            }
            _ => Accept::Pass,
        }
    }

    fn finish(&mut self, default: &[Agent]) -> Vec<Entry> {
        std::mem::take(&mut self.0)
            .into_iter()
            .map(|((source, agents), names)| Entry::Subagent {
                source,
                names: sorted(names),
                agents: own(agents, default),
            })
            .collect()
    }
}

/// The file name is what a copy finds the definition by.
fn file_stem(item: &Augmentation) -> String {
    let file = item.children.iter().find_map(|c| c.strip_prefix("file: "));
    let stem = file.and_then(|f| f.rsplit_once('.').map(|(s, _)| s));
    stem.unwrap_or(&item.name).to_string()
}
