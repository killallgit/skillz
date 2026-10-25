//! One logical item per thing installed, however many agents see it: a subagent in both
//! `.claude/agents` and `.codex/agents`, or the same hook in two agents' config, is one
//! `Installed` whose `members` are the copies. Pure; discovery stays per agent.

mod key;
mod tag;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use self::key::Key;
pub use self::tag::tag;
use crate::harness::model::{Details, Installed};

/// `items` with every set of copies of one thing folded into its first copy's place.
pub fn merge(items: Vec<Installed>) -> Vec<Installed> {
    let mut slots: Vec<Vec<Installed>> = Vec::new();
    let mut slot_of: HashMap<Key, usize> = HashMap::new();
    for item in items {
        let slot = *slot_of.entry(Key::of(&item)).or_insert_with(|| {
            slots.push(Vec::new());
            slots.len() - 1
        });
        slots[slot].push(item);
    }
    slots.into_iter().map(fold).collect()
}

/// The copies as one item: the first copy's identity, every agent, every removal step in order.
fn fold(mut copies: Vec<Installed>) -> Installed {
    if copies.len() == 1 {
        return copies.remove(0);
    }
    let mut merged = copies[0].clone();
    merged.members = copies.iter().flat_map(|c| c.parts().to_vec()).collect();
    merged.agents = copies.iter().flat_map(|c| c.agents.clone()).collect();
    merged.agents.sort();
    merged.agents.dedup();
    merged.removal = copies.iter().flat_map(|c| c.removal.clone()).collect();
    merged.children = distinct(copies.iter().flat_map(|c| c.children.clone()));
    merged.hooks = distinct(copies.iter().flat_map(|c| c.hooks.clone()));
    if let Details::Plugin { skills, .. } = &mut merged.details {
        *skills = distinct(
            copies
                .iter()
                .flat_map(|copy| copy.plugin_skills().map(str::to_owned)),
        );
    }
    merged
}

fn distinct<T: PartialEq>(all: impl Iterator<Item = T>) -> Vec<T> {
    let mut kept: Vec<T> = Vec::new();
    for one in all {
        if !kept.contains(&one) {
            kept.push(one);
        }
    }
    kept
}
