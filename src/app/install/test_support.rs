//! Fakes and fixtures shared by the install tests.

use std::cell::RefCell;

use crate::app::prompt::Prompter;
use skillz_harness::model::{Agent, Scope};

pub use skillz_harness::test_support::*;

use super::{InstallRequest, SkillSelection};

pub struct ScriptedPrompter {
    picks: Vec<usize>,
    confirm: bool,
    pub offered: RefCell<Vec<Vec<String>>>,
}

impl ScriptedPrompter {
    pub fn new(picks: &[usize], confirm: bool) -> Self {
        ScriptedPrompter {
            picks: picks.to_vec(),
            confirm,
            offered: RefCell::default(),
        }
    }
}

impl Prompter for ScriptedPrompter {
    fn multi_select(&self, _title: &str, options: &[String]) -> anyhow::Result<Vec<usize>> {
        self.offered.borrow_mut().push(options.to_vec());
        Ok(self.picks.clone())
    }
    fn confirm(&self, _question: &str) -> anyhow::Result<bool> {
        Ok(self.confirm)
    }
}

/// A request run from `fx`'s project dir.
pub fn req(fx: &Fixture, source: &str, selection: SkillSelection) -> InstallRequest {
    InstallRequest {
        source: source.into(),
        cwd: fx.env.project.clone(),
        agents: vec![Agent::Claude],
        scope: Scope::Project,
        selection,
    }
}
