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
pub use standing::{Standing, compare, holds, loaded};
pub use switch::Switch;

use crate::app::stale::Probe;
use skillz_harness::agents::{self, Harness};
use skillz_harness::env::Env;
use skillz_harness::model::{Installed, Scope};
use skillz_harness::runner::CommandRunner;

/// Everything a switch of one scope works with.
pub struct Applier<'a> {
    pub adapters: &'a [Box<dyn Harness>],
    pub env: &'a Env,
    pub scope: Scope,
    /// Tells whether a local source still exists before anything is installed from it.
    pub probe: &'a dyn Probe,
    /// Fetches a skill entry's source at plan time to see whether its marketplace supplies it.
    pub runner: &'a dyn CommandRunner,
}

impl Applier<'_> {
    fn installed(&self) -> anyhow::Result<Vec<Installed>> {
        agents::inventory(self.adapters, self.env, &[self.scope]).complete()
    }
}
