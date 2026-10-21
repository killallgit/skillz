use std::path::Path;

use crate::clone::{check_source, with_tmp_clone};
use crate::runner::{CommandRunner, run_checked};

/// Where an install source is fetched from and how.
pub(super) struct Fetch<'a> {
    pub source: &'a str,
    /// What a relative local `source` is relative to.
    pub cwd: &'a Path,
    /// Where the clone commands run.
    pub project: &'a Path,
    pub runner: &'a dyn CommandRunner,
}

/// Run `f(tree, source)` on the install source: `<cwd>/<source>` when it exists, else a
/// `git clone --depth 1` into a temp dir removed afterwards. `source` is what to hand to
/// `npx`/`claude`: the absolute local path, or the source as given (never the tmp clone).
pub(super) fn with_source(
    fetch: &Fetch,
    f: impl FnOnce(&Path, &str) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let source = fetch.source;
    check_source(source)?;
    let local = fetch.cwd.join(source);
    if local.exists() {
        return f(&local, &local.to_string_lossy());
    }
    with_tmp_clone(
        source,
        |(program, args)| run_checked(fetch.runner, program, args, fetch.project),
        |tmp| f(tmp, source),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::Env;
    use crate::install::test_support::*;
    use std::cell::Cell;

    fn fetch<'a>(
        source: &'a str,
        fx: &'a Fixture,
        env: &'a Env,
        runner: &'a dyn CommandRunner,
    ) -> Fetch<'a> {
        Fetch {
            source,
            cwd: &fx.env.project,
            project: &env.project,
            runner,
        }
    }

    #[test]
    fn local_source_is_used_in_place_as_absolute_path() {
        let fx = project_with(skills_fixture);
        let runner = FakeRunner::new(|_| {});
        let mut got = None;
        with_source(&fetch("repo", &fx, &fx.env, &runner), |tree, source| {
            got = Some((tree.to_path_buf(), source.to_string()));
            Ok(())
        })
        .unwrap();
        let repo = fx.env.project.join("repo");
        assert_eq!(got, Some((repo.clone(), repo.to_string_lossy().into())));
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn local_source_resolves_against_cwd_not_project() {
        let fx = project_with(skills_fixture);
        let elsewhere = tempfile::tempdir().unwrap();
        let env = Env {
            project: elsewhere.path().into(),
            ..fx.env.clone()
        };
        let runner = FakeRunner::new(|_| {});
        let mut got = None;
        with_source(&fetch("repo", &fx, &env, &runner), |tree, _| {
            got = Some(tree.to_path_buf());
            Ok(())
        })
        .unwrap();
        assert_eq!(got, Some(fx.env.project.join("repo")));
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn option_like_or_empty_source_is_err_and_runs_nothing() {
        let fx = project_with(|_| {});
        let runner = FakeRunner::new(|_| {});
        for source in ["", "  ", "--upload-pack=x"] {
            let res = with_source(&fetch(source, &fx, &fx.env, &runner), |_, _| Ok(()));
            assert!(res.is_err());
        }
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn clone_failure_is_err_and_skips_install() {
        let fx = project_with(|_| {});
        let runner = FakeRunner::new(skills_fixture).failing("git");
        let called = Cell::new(false);
        let res = with_source(&fetch("org/repo", &fx, &fx.env, &runner), |_, _| {
            called.set(true);
            Ok(())
        });
        assert!(res.is_err());
        assert!(!called.get());
        assert_eq!(runner.calls().len(), 1);
    }

    #[test]
    fn clone_failure_error_hides_url_credentials() {
        let fx = project_with(|_| {});
        let runner = FakeRunner::new(skills_fixture).failing("git");
        let err = with_source(
            &fetch("https://u:s3cret@x.dev/o/r.git", &fx, &fx.env, &runner),
            |_, _| Ok(()),
        )
        .unwrap_err();
        assert!(!format!("{err:#}").contains("s3cret"));
    }
}
