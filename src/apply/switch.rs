//! The difference between what a scope has loaded and what a profile wants. Pure data: nothing
//! here reads or changes the machine.

use crate::action::Action;
use crate::model::{Augmentation, Kind};
use crate::profile::Entry;
use crate::provision::Unsupported;
use crate::remove::Plan;

use super::ledger::{Loaded, Owned};

/// What moving a scope to a target profile involves. Build it with `Applier::plan`, show it,
/// then `Applier::run` it.
#[derive(Debug)]
pub struct Switch {
    pub from: Option<String>,
    /// `None` to unapply.
    pub to: Option<String>,
    /// Entries in both: left as they are.
    pub keep: Vec<Loaded>,
    /// Entries only in the old profile.
    pub drop: Vec<Loaded>,
    /// The installed items the dropped entries added, to remove first.
    pub removals: Vec<Augmentation>,
    /// Entries only in the new profile, each with its install steps.
    pub installs: Vec<(Entry, Vec<Action>)>,
    pub unsupported: Vec<Unsupported>,
}

impl Switch {
    pub fn is_empty(&self) -> bool {
        self.drop.is_empty() && self.installs.is_empty() && self.unsupported.is_empty()
    }

    /// The removal and install steps in the order `run` takes them.
    pub fn steps(&self) -> Vec<Action> {
        let refs: Vec<&Augmentation> = self.removals.iter().collect();
        Plan::new(&refs, &[])
            .steps
            .into_iter()
            .chain(self.installs.iter().flat_map(|(_, steps)| steps.clone()))
            .collect()
    }
}

/// How the loaded entries and the wanted ones overlap.
pub struct Diff {
    pub keep: Vec<Loaded>,
    pub drop: Vec<Loaded>,
    /// Wanted entries not loaded yet, each once.
    pub add: Vec<Entry>,
}

pub fn diff_entries(current: &[Loaded], wanted: &[Entry]) -> Diff {
    let (mut keep, mut drop): (Vec<Loaded>, Vec<Loaded>) = current
        .iter()
        .cloned()
        .partition(|l| wanted.contains(&l.entry));
    let add = wanted
        .iter()
        .enumerate()
        .filter(|(i, e)| !current.iter().any(|l| &l.entry == *e) && !wanted[..*i].contains(e))
        .map(|(_, e)| e.clone())
        .collect();
    keep_shared_marketplaces(&mut keep, &mut drop);
    Diff { keep, drop, add }
}

/// A marketplace is added once, by whichever entry came first. When that entry goes but another
/// kept entry still has a plugin from it, the marketplace stays and the kept entry owns it.
fn keep_shared_marketplaces(keep: &mut [Loaded], drop: &mut [Loaded]) {
    for dropped in drop.iter_mut() {
        let items = std::mem::take(&mut dropped.items);
        let (shared, rest): (Vec<Owned>, Vec<Owned>) = items
            .into_iter()
            .partition(|o| o.kind == Kind::Marketplace && uses_marketplace(keep, &o.name));
        dropped.items = rest;
        for item in shared {
            if let Some(owner) = keep.iter_mut().find(|k| draws_on(&k.entry, &item.name)) {
                owner.items.push(item);
            }
        }
    }
}

fn uses_marketplace(entries: &[Loaded], marketplace: &str) -> bool {
    entries.iter().any(|l| draws_on(&l.entry, marketplace))
}

fn draws_on(entry: &Entry, wanted: &str) -> bool {
    matches!(entry, Entry::Plugin { marketplace, .. } if marketplace == wanted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Agent;

    fn plugin(name: &str) -> Entry {
        Entry::Plugin {
            source: "o/mp".into(),
            marketplace: "mp".into(),
            name: name.into(),
            agents: vec![Agent::Claude],
        }
    }

    fn skill(name: &str) -> Entry {
        Entry::Skill {
            source: "o/r".into(),
            names: vec![name.into()],
            agents: vec![Agent::Claude],
        }
    }

    fn owned(name: &str, kind: Kind) -> Owned {
        Owned {
            id: format!("{kind:?}:{name}"),
            name: name.into(),
            kind,
        }
    }

    fn loaded(entry: Entry, items: Vec<Owned>) -> Loaded {
        Loaded { entry, items }
    }

    #[test]
    fn entries_are_kept_dropped_or_added_by_comparing_them() {
        let current = [loaded(skill("a"), vec![]), loaded(skill("b"), vec![])];
        let wanted = [skill("b"), skill("c"), skill("c")];

        let diff = diff_entries(&current, &wanted);

        assert_eq!(diff.keep, [current[1].clone()]);
        assert_eq!(diff.drop, [current[0].clone()]);
        assert_eq!(diff.add, [skill("c")]);
    }

    #[test]
    fn a_marketplace_stays_with_the_kept_entry_that_still_uses_it() {
        let current = [
            loaded(
                plugin("a"),
                vec![owned("mp", Kind::Marketplace), owned("a", Kind::Plugin)],
            ),
            loaded(plugin("b"), vec![owned("b", Kind::Plugin)]),
        ];

        let diff = diff_entries(&current, &[plugin("b")]);

        assert_eq!(diff.drop[0].items, [owned("a", Kind::Plugin)]);
        assert_eq!(diff.keep[0].items.len(), 2);
    }

    #[test]
    fn a_lone_plugins_marketplace_goes_with_it() {
        let current = [loaded(
            plugin("a"),
            vec![owned("mp", Kind::Marketplace), owned("a", Kind::Plugin)],
        )];

        let diff = diff_entries(&current, &[]);

        assert_eq!(diff.drop[0].items.len(), 2);
    }

    #[test]
    fn an_empty_switch_has_nothing_to_show_or_run() {
        let switch = Switch {
            from: None,
            to: None,
            keep: vec![],
            drop: vec![],
            removals: vec![],
            installs: vec![],
            unsupported: vec![],
        };

        assert!(switch.is_empty());
        assert!(switch.steps().is_empty());
    }
}
