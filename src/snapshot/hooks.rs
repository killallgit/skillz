use std::collections::BTreeMap;

use super::{Accept, Collector, Reason, Skipped, merge_agents, own};
use crate::model::{Agent, Augmentation, Kind};
use crate::profile::Entry;

/// One hook handler: event, matcher, command.
type HookKey = (String, Option<String>, String);

/// Hook handlers read back from the `event [matcher]: command` children of a hook group.
#[derive(Default)]
pub(super) struct Hooks(BTreeMap<HookKey, Vec<Agent>>);

impl Collector for Hooks {
    fn accept(&mut self, item: &Augmentation, agents: Vec<Agent>) -> Accept {
        if item.kind != Kind::Hook {
            return Accept::Pass;
        }
        let keys: Option<Vec<HookKey>> = item.children.iter().map(|c| parse_child(c)).collect();
        match keys.filter(|k| !k.is_empty()) {
            Some(keys) => {
                for key in keys {
                    merge_agents(self.0.entry(key).or_default(), agents.clone());
                }
                Accept::Taken
            }
            None => Accept::Skipped(Skipped::new(item, Reason::Unreadable)),
        }
    }

    fn finish(&mut self, default: &[Agent]) -> Vec<Entry> {
        std::mem::take(&mut self.0)
            .into_iter()
            .map(|((event, matcher, command), agents)| Entry::Hook {
                event,
                matcher,
                command,
                agents: own(agents, default),
            })
            .collect()
    }
}

fn parse_child(child: &str) -> Option<HookKey> {
    let (head, command) = child.split_once(": ")?;
    let (event, matcher) = match head.split_once(" [") {
        Some((event, rest)) => (event, Some(rest.strip_suffix(']')?.to_string())),
        None => (head, None),
    };
    Some((event.to_string(), matcher, command.to_string()))
}
