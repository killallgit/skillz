//! Bring a scope to a profile: install what the inventory lacks of its entries and, when asked
//! to be exact, remove what it holds that the profile does not want. Which profile a directory
//! has loaded is never stored; it is read off the inventory (`standing`).

mod plan;
mod run;
mod run_each;
mod standing;
mod switch;
#[cfg(test)]
mod tests;

pub use run::Done;
pub use run_each::Each;
pub use standing::{Standing, compare, holds, loaded};
pub use switch::Switch;

use crate::agents::{self, AgentAdapter};
use crate::env::Env;
use crate::model::{Augmentation, Scope};

/// Everything a switch of one scope works with.
pub struct Applier<'a> {
    pub adapters: &'a [Box<dyn AgentAdapter>],
    pub env: &'a Env,
    pub scope: Scope,
}

impl Applier<'_> {
    fn installed(&self) -> anyhow::Result<Vec<Augmentation>> {
        agents::inventory(self.adapters, self.env, &[self.scope]).complete()
    }
}
