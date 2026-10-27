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

    #[test]
    fn library_skills_are_reinspected_and_install_the_native_package() {
        use crate::app::install::test_support::mattpocock_fixture;
        use crate::app::workspace::Act;
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let runner = FakeRunner::new(mattpocock_fixture);
        let prompter = ScriptedPrompter::new(&[], true);
        let deps = Deps::for_test(home.path(), registry(), (&runner, &prompter));
        let env = skillz_harness::env::Env::new(home.path().into(), project.path().into());
        let session = Session::open(&deps, &env);

        session
            .perform(Act::LibraryInstall {
                scope: Scope::Project,
                entry: skillz_harness::Spec::Skill {
                    source: "mattpocock/skills".into(),
                    names: vec!["tdd".into()],
                    agents: vec![Agent::Claude, Agent::Codex],
                },
            })
            .unwrap();

        let calls = runner.calls();
        assert_eq!(
            calls.iter().map(|call| call.0.as_str()).collect::<Vec<_>>(),
            ["git", "claude", "claude", "codex"]
        );
        assert_eq!(
            calls[2].1,
            vec![
                "plugin",
                "install",
                "mattpocock-skills@mattpocock",
                "--scope",
                "local"
            ]
        );
        assert!(!home.path().join(".codex/config.toml").exists());
        let doc: toml_edit::DocumentMut =
            std::fs::read_to_string(project.path().join(".codex/config.toml"))
                .unwrap()
                .parse()
                .unwrap();
        assert_eq!(
            doc["plugins"]["mattpocock-skills@mattpocock"]["enabled"].as_bool(),
            Some(true)
        );
    }
}
