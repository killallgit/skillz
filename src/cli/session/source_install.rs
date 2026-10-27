//! Installing what a repo or path offers, then remembering it in the library.

use anyhow::Context;

use super::{Notice, Session};
use crate::app::install::{self, InstallRequest};
use crate::cli::recording::record_since;
use crate::cli::scope::{complete_installed, project_env};

impl Session<'_, '_> {
    /// Install `request`'s source through the agents' own installers, then record what arrived.
    /// A failed install still records whatever it left behind.
    pub(in crate::cli) fn install_source(
        &self,
        request: &InstallRequest,
    ) -> anyhow::Result<Option<Notice>> {
        let env = project_env(self.deps, &self.project);
        let ctx = install::Ctx {
            adapters: &self.deps.adapters,
            env: &env,
            runner: self.deps.runner,
            prompter: self.deps.prompter,
        };
        let before = complete_installed(self.deps, &env, request.scope)?;
        let result = install::install(&ctx, request);
        let recorded = record_since(self.deps, &self.store, (&env, request.scope), &before);
        let installed = result?;
        recorded.context("installed, but the library was not updated")?;
        Ok(installed.then(|| Notice::SourceInstalled(request.source.clone())))
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::app::install::SkillSelection;
    use crate::app::install::test_support::{FakeRunner, ScriptedPrompter, marketplace_fixture};
    use crate::app::{profile, store};
    use crate::cli::deps::Deps;
    use skillz_harness::agents::registry;
    use skillz_harness::model::{Agent, Scope};

    fn request(dir: &Path) -> InstallRequest {
        InstallRequest {
            source: "o/r".into(),
            cwd: dir.to_path_buf(),
            agents: vec![Agent::Claude],
            scope: Scope::Global,
            selection: SkillSelection::Interactive,
        }
    }

    fn install_with(approve: bool) -> (Option<Notice>, Vec<String>) {
        let dir = tempfile::tempdir().unwrap();
        let runner = FakeRunner::new(marketplace_fixture);
        let prompter = ScriptedPrompter::new(&[0], approve);
        let deps = Deps::for_test(dir.path(), registry(), (&runner, &prompter));
        let session = Session {
            deps: &deps,
            profiles: profile::Store::new(dir.path().join("store.json")),
            store: store::Store::new(dir.path().join("store.json")),
            project: dir.path().to_path_buf(),
        };
        let notice = session.install_source(&request(dir.path())).unwrap();
        let programs = runner.calls().into_iter().map(|call| call.0).collect();
        (notice, programs)
    }

    #[test]
    fn an_approved_plan_runs_and_is_reported() {
        let (notice, programs) = install_with(true);

        assert!(matches!(notice, Some(Notice::SourceInstalled(source)) if source == "o/r"));
        assert_eq!(programs, ["git", "claude", "claude"]);
    }

    #[test]
    fn a_declined_plan_reports_nothing_and_runs_only_the_clone() {
        let (notice, programs) = install_with(false);

        assert!(notice.is_none());
        assert_eq!(programs, ["git"]);
    }
}
