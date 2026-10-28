//! Fakes and fixtures shared by the install tests.

use std::cell::RefCell;

use crate::app::prompt::Prompter;
use crate::app::review::{Group, PlanView, Verdict};
use skillz_harness::model::{Agent, Scope};

pub use skillz_harness::test_support::*;

pub fn mattpocock_fixture(root: &std::path::Path) {
    skill(root, "skills/engineering/tdd");
    skill(root, "skills/engineering/code-review");
    std::fs::create_dir_all(root.join(".claude-plugin")).unwrap();
    std::fs::write(
        root.join(".claude-plugin/marketplace.json"),
        r#"{"name":"mattpocock","plugins":[{"name":"mattpocock-skills","source":"./"}]}"#,
    )
    .unwrap();
}

use super::{InstallRequest, SkillSelection};

pub struct ScriptedPrompter {
    picks: Vec<usize>,
    confirm: bool,
    pub offered: RefCell<Vec<Vec<String>>>,
    /// Each plan shown, as how many lines it listed under each group.
    pub plans: RefCell<Vec<Vec<(Group, usize)>>>,
}

impl ScriptedPrompter {
    pub fn new(picks: &[usize], confirm: bool) -> Self {
        ScriptedPrompter {
            picks: picks.to_vec(),
            confirm,
            offered: RefCell::default(),
            plans: RefCell::default(),
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
    fn plan(&self, view: &PlanView) -> anyhow::Result<Verdict> {
        let shape = view.groups.iter().map(|(g, lines)| (*g, lines.len()));
        self.plans.borrow_mut().push(shape.collect());
        Ok(if self.confirm {
            Verdict::Apply
        } else {
            Verdict::Cancel
        })
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
        npx: false,
    }
}
