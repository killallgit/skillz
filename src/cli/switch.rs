//! Switching a project's profile from the workspace: review the plan on screen, hand the
//! terminal to the tools while they run, then report what happened.

use std::path::Path;

use super::deps::Deps;
use super::scope::project_env;
use super::use_profile::{Switching, Target, use_profile};
use crate::model::Scope;
use crate::prompt::Prompter;
use crate::tree::Node;

/// Says yes to every question: the plan was already reviewed on screen.
struct Reviewed<'a>(&'a dyn Prompter);

impl Prompter for Reviewed<'_> {
    fn multi_select(&self, title: &str, options: &[String]) -> anyhow::Result<Vec<usize>> {
        self.0.multi_select(title, options)
    }

    fn select_items(&self, title: &str, nodes: &[Node]) -> anyhow::Result<Vec<usize>> {
        self.0.select_items(title, nodes)
    }

    fn confirm(&self, _: &str) -> anyhow::Result<bool> {
        Ok(true)
    }
}

/// Load `profile` into `project` (or unload with `None`) once the user approves the plan.
pub(super) fn switch(deps: &Deps, project: &Path, profile: Option<&str>) -> anyhow::Result<()> {
    let env = project_env(deps, project);
    let target = Target {
        env: &env,
        scope: Scope::Project,
    };
    let mut plan = Vec::new();
    let preview = Switching {
        profile,
        dry_run: true,
    };
    use_profile(deps, &target, &preview, &mut plan)?;
    if !deps
        .prompter
        .review("Switch", &String::from_utf8_lossy(&plan), "Proceed?")?
    {
        return Ok(());
    }
    deps.prompter.release_terminal();
    let reviewed = Reviewed(deps.prompter);
    let approved = deps.with_prompter(&reviewed);
    let mut log = Vec::new();
    let run = Switching {
        profile,
        dry_run: false,
    };
    use_profile(&approved, &target, &run, &mut log)?;
    deps.prompter
        .report("Switched", &String::from_utf8_lossy(&log))
}
