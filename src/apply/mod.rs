//! Bring a scope to a profile: install what the inventory lacks of its entries and, when asked
//! to be exact, remove what it holds that the profile does not want. What a directory holds is
//! always read off the inventory (`standing`); the profile last applied is only remembered, by
//! name, so the workspace can say what the directory drifted from.

mod drift;
#[cfg(test)]
mod each_tests;
mod plan;
mod run;
mod source_gone;
mod standing;
mod switch;
#[cfg(test)]
mod tests;

pub use drift::{Drift, drift};
pub use run::{Report, Watch};
pub use standing::{Standing, compare, holds, loaded, only_wanted_by};
pub use switch::Switch;

use crate::agents::{self, AgentAdapter};
use crate::env::Env;
use crate::model::{Augmentation, Scope};
use crate::stale::Probe;

/// Everything a switch of one scope works with.
pub struct Applier<'a> {
    pub adapters: &'a [Box<dyn AgentAdapter>],
    pub env: &'a Env,
    pub scope: Scope,
    /// Tells whether a local source still exists before anything is installed from it.
    pub probe: &'a dyn Probe,
}

impl Applier<'_> {
    fn installed(&self) -> anyhow::Result<Vec<Augmentation>> {
        agents::inventory(self.adapters, self.env, &[self.scope]).complete()
    }
}
