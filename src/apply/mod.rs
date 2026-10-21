//! Load a profile into a scope, swap it for another, or take it out. What each entry added is
//! found by reading the inventory before and after it, and recorded in a ledger. A switch
//! compares the ledger's entries with the target's: dropped entries lose exactly the items they
//! added, kept entries are left alone, new ones are installed. Nothing that was already there or
//! was installed by hand since is touched.

mod ledger;
mod plan;
mod run;
mod switch;
#[cfg(test)]
mod tests;

pub use ledger::{Ledger, Loaded, Owned, Record};
pub use run::Done;
pub use switch::Switch;

use crate::agents::{self, AgentAdapter};
use crate::env::Env;
use crate::model::{Augmentation, Scope};

/// Everything a switch of one scope works with.
pub struct Applier<'a> {
    pub adapters: &'a [Box<dyn AgentAdapter>],
    pub env: &'a Env,
    pub scope: Scope,
    pub ledger: &'a Ledger,
}

impl Applier<'_> {
    fn ledger_key(&self) -> String {
        Ledger::key(self.env, self.scope)
    }

    fn installed(&self) -> Vec<Augmentation> {
        agents::inventory(self.adapters, self.env, &[self.scope]).items
    }
}
