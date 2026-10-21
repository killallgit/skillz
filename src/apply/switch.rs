//! What bringing a project to a profile involves. Pure data: nothing here reads or changes the
//! machine.

use crate::action::Action;
use crate::model::Augmentation;
use crate::profile::Entry;
use crate::provision::Unsupported;
use crate::remove::Plan;

/// Build it with `Applier::plan`, show it, then `Applier::run` it.
#[derive(Debug)]
pub struct Switch {
    pub to: String,
    /// Wanted entries already in place.
    pub present: Vec<Entry>,
    /// Installed items to remove first: the ones the profile does not want, in exact mode.
    pub removals: Vec<Augmentation>,
    /// The ids among `removals` that no profile could install again.
    pub unsourced: Vec<String>,
    /// Entries (or the parts of them) not in place yet, each with its install steps.
    pub installs: Vec<(Entry, Vec<Action>)>,
    pub unsupported: Vec<Unsupported>,
}

impl Switch {
    pub fn is_empty(&self) -> bool {
        self.removals.is_empty() && self.installs.is_empty() && self.unsupported.is_empty()
    }

    /// The removal and install steps in the order `run` takes them.
    pub fn steps(&self) -> Vec<Action> {
        Plan::from_owned(&self.removals)
            .steps
            .into_iter()
            .chain(self.installs.iter().flat_map(|(_, steps)| steps.clone()))
            .collect()
    }

    pub fn is_unsourced(&self, item: &Augmentation) -> bool {
        self.unsourced.contains(&item.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_switch_has_nothing_to_show_or_run() {
        let switch = Switch {
            to: "p".into(),
            present: vec![],
            removals: vec![],
            unsourced: vec![],
            installs: vec![],
            unsupported: vec![],
        };

        assert!(switch.is_empty());
        assert!(switch.steps().is_empty());
    }
}
