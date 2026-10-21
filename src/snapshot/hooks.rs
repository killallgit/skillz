use std::collections::BTreeMap;

use super::{Accept, Collector, Reason, Skipped, merge_agents, own};
use crate::model::{Agent, Augmentation, Kind};
use crate::profile::Entry;

/// One hook handler: event, matcher, command.
type HookKey = (String, Option<String>, String);

/// Hook handlers carried by hook groups.
#[derive(Default)]
pub(super) struct Hooks(BTreeMap<HookKey, Vec<Agent>>);

impl Collector for Hooks {
    fn accept(&mut self, item: &Augmentation, agents: Vec<Agent>) -> Accept {
        if item.kind != Kind::Hook {
            return Accept::Pass;
        }
        if item.hooks.is_empty() {
            return Accept::Skipped(Skipped::new(item, Reason::Unreadable));
        }
        for hook in &item.hooks {
            let key = (
                hook.event.clone(),
                hook.matcher.clone(),
                hook.command.clone(),
            );
            merge_agents(self.0.entry(key).or_default(), agents.clone());
        }
        Accept::Taken
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
