//! Resolve a source directory or inspect a temporary git clone.

use std::path::Path;

use anyhow::Context;

use crate::redact::redact_text;
use crate::runner::{Cmd, CommandRunner};

/// Sources reach git/npx/claude as positional args, so a leading `-` would be read as an option.
fn check_source(source: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !source.trim().is_empty() && !source.starts_with('-'),
        "invalid source: {}",
        redact_text(source)
    );
    Ok(())
}

/// A GitHub shorthand is exactly `<owner>/<repo>`: anything else (`../x`, `~/x`, `a/b/c`) is
/// not a repository name and must not reach git.
fn check_repo_name(source: &str) -> anyhow::Result<()> {
    let is_url = source.contains("://") || source.starts_with("git@");
    let is_name = |part: &str| {
        !part.is_empty() && part != "." && part != ".." && !part.starts_with(['-', '~'])
    };
    let is_owner_and_repo = source
        .split_once('/')
        .is_some_and(|(owner, repo)| is_name(owner) && is_name(repo) && !repo.contains('/'));
    anyhow::ensure!(
        is_url || (is_owner_and_repo && !source.contains('\\')),
        "invalid repository name: {}",
        redact_text(source)
    );
    Ok(())
}

/// Resolve a local directory or inspect a temporary clone. The callback receives the stable
/// install source as well as the fetched tree, so temporary paths are never persisted.
pub fn with_source<T>(
    source: &str,
    cwd: &Path,
    runner: &dyn CommandRunner,
    inspect: impl FnOnce(&Path, &str) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    check_source(source)?;
    let local = cwd.join(source);
    match std::fs::metadata(&local) {
        Ok(metadata) => {
            anyhow::ensure!(
                metadata.is_dir(),
                "source is not a directory: {}",
                local.display()
            );
            let local = local.canonicalize()?;
            return inspect(&local, &local.to_string_lossy());
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e).with_context(|| format!("reading source {}", redact_text(source))),
    }
    check_repo_name(source)?;
    let temporary = tempfile::Builder::new()
        .prefix("skillz-install-")
        .tempdir()?;
    let (program, args) = clone_command(source, temporary.path());
    let output = runner.run_quiet(&program, &args, cwd)?;
    anyhow::ensure!(
        output.success,
        "cloning {} failed: {}",
        redact_text(source),
        redact_text(output.stderr.trim())
    );
    inspect(temporary.path(), source)
}

pub(crate) fn clone_url(source: &str) -> String {
    if source.contains("://") || source.starts_with("git@") {
        source.to_string()
    } else {
        format!("https://github.com/{source}.git")
    }
}

/// A git source as `host/owner/repo`, whichever way it is spelled: shorthand, https, ssh or scp
/// form, with or without `.git`, a trailing slash or capitals.
pub fn repo_identity(source: &str) -> String {
    let url = clone_url(source).to_ascii_lowercase();
    let rest = url.split_once("://").map_or(url.as_str(), |(_, rest)| rest);
    let rest = match rest.split_once('@') {
        Some((user, after)) if !user.contains('/') => after,
        _ => rest,
    };
    rest.replacen(':', "/", 1)
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .trim_end_matches('/')
        .to_string()
}

/// A repository source as its `repo_identity`; a local path exactly as spelled, since only the
/// filesystem knows whether two spellings of a path are the same directory.
pub fn source_identity(source: &str) -> String {
    match check_repo_name(source) {
        Ok(()) => repo_identity(source),
        Err(_) => source.to_string(),
    }
}

fn clone_command(source: &str, dest: &Path) -> Cmd {
    let args = [
        "clone",
        "--depth",
        "1",
        "--",
        &clone_url(source),
        &dest.to_string_lossy(),
    ]
    .map(String::from)
    .to_vec();
    ("git".into(), args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clone_url_keeps_full_urls() {
        assert_eq!(clone_url("https://x.dev/a/b.git"), "https://x.dev/a/b.git");
        assert_eq!(
            clone_url("git@github.com:a/b.git"),
            "git@github.com:a/b.git"
        );
        assert_eq!(clone_url("a/b"), "https://github.com/a/b.git");
    }

    #[test]
    fn option_looking_and_blank_sources_are_refused() {
        // both would be read by git/npx as options or nothing
        assert!(check_source("-x").is_err() && check_source(" ").is_err());
        assert!(check_source("a/b").is_ok());
    }
}

#[cfg(test)]
mod source_tests {
    use super::*;
    use crate::test_support::*;
    use std::cell::Cell;

    #[test]
    fn local_source_is_used_in_place_as_absolute_path() {
        let fx = project_with(skills_fixture);
        let runner = FakeRunner::new(|_| {});
        let mut got = None;
        with_source("repo", &fx.env.project, &runner, |tree, source| {
            got = Some((tree.to_path_buf(), source.to_string()));
            Ok(())
        })
        .unwrap();
        let repo = fx.env.project.join("repo").canonicalize().unwrap();
        assert_eq!(got, Some((repo.clone(), repo.to_string_lossy().into())));
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn local_source_resolves_against_cwd_not_project() {
        let fx = project_with(skills_fixture);
        let runner = FakeRunner::new(|_| {});
        let mut got = None;
        with_source("repo", &fx.env.project, &runner, |tree, _| {
            got = Some(tree.to_path_buf());
            Ok(())
        })
        .unwrap();
        assert_eq!(
            got,
            Some(fx.env.project.join("repo").canonicalize().unwrap())
        );
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn option_like_or_empty_source_is_err_and_runs_nothing() {
        let fx = project_with(|_| {});
        let runner = FakeRunner::new(|_| {});
        for source in ["", "  ", "--upload-pack=x"] {
            let res = with_source(source, &fx.env.project, &runner, |_, _| Ok(()));
            assert!(res.is_err());
        }
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn anything_but_owner_slash_repo_is_refused_and_runs_nothing() {
        let fx = project_with(|_| {});
        let runner = FakeRunner::new(|_| {});
        let sources = [
            "../x",
            "a/../b",
            "./a/b",
            "~/x",
            "~o/r",
            "o/~r",
            "o/-r",
            "o/r/extra",
            "o",
            "o//r",
            "/o/r",
            "o\\r",
            "o/r\\x",
            "o\\x/r",
            "o/",
            "/r",
            "./o",
        ];
        for source in sources {
            let res = with_source(source, &fx.env.project, &runner, |_, _| Ok(()));
            assert!(res.is_err(), "{source}");
        }
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn owner_slash_repo_and_urls_are_cloned_unchanged() {
        let fx = project_with(|_| {});
        let runner = FakeRunner::new(|_| {});
        let sources = ["o/r", "https://x.dev/o/r.git", "git@x.dev:o/r.git"];
        for source in sources {
            with_source(source, &fx.env.project, &runner, |_, _| Ok(())).unwrap();
        }
        let cloned: Vec<String> = runner
            .calls()
            .iter()
            .map(|(_, args, _)| args[4].clone())
            .collect();
        assert_eq!(
            cloned,
            [
                "https://github.com/o/r.git",
                "https://x.dev/o/r.git",
                "git@x.dev:o/r.git"
            ]
        );
    }

    #[test]
    fn a_local_directory_at_a_deep_relative_path_is_used_in_place() {
        let directory = tempfile::tempdir().unwrap();
        let deep = directory.path().join("a/b/c");
        std::fs::create_dir_all(&deep).unwrap();
        let runner = FakeRunner::new(|_| {});
        let mut used = None;

        with_source("a/b/c", directory.path(), &runner, |tree, _| {
            used = Some(tree.to_path_buf());
            Ok(())
        })
        .unwrap();

        assert_eq!(used, Some(deep.canonicalize().unwrap()));
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn clone_failure_is_err_and_skips_install() {
        let fx = project_with(|_| {});
        let runner = FakeRunner::new(skills_fixture).failing("git");
        let called = Cell::new(false);
        let res = with_source("org/repo", &fx.env.project, &runner, |_, _| {
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
            "https://u:s3cret@x.dev/o/r.git",
            &fx.env.project,
            &runner,
            |_, _| Ok(()),
        )
        .unwrap_err();
        assert!(!format!("{err:#}").contains("s3cret"));
    }
    #[test]
    fn a_file_source_fails_before_a_clone_or_callback() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("source"), "not a directory").unwrap();
        let runner = FakeRunner::new(|_| {});
        let called = Cell::new(false);

        let result = with_source("source", directory.path(), &runner, |_, _| {
            called.set(true);
            Ok(())
        });

        assert!(result.is_err());
        assert!(!called.get() && runner.calls().is_empty());
    }
}
