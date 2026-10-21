use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::env::Env;
use crate::runner::CommandRunner;

use super::Cmd;

/// Run `f(tree, source)` on the install source: `<cwd>/<source>` when it exists, else a
/// `git clone --depth 1` into a temp dir removed afterwards. `source` is what to hand to
/// `npx`/`claude`: the absolute local path, or the source as given (never the tmp clone).
pub(super) fn with_source(
    source: &str,
    cwd: &Path,
    env: &Env,
    runner: &dyn CommandRunner,
    f: impl FnOnce(&Path, &str) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    // Sources are passed as positional args to git/npx/claude; a leading `-` would be an option.
    if source.trim().is_empty() || source.starts_with('-') {
        anyhow::bail!("invalid source: {source}");
    }
    let local = cwd.join(source);
    if local.exists() {
        return f(&local, &local.to_string_lossy());
    }
    let tmp = std::env::temp_dir().join(unique_name());
    let (program, args) = clone_command(source, &tmp);
    let res = run_checked(runner, &program, &args, &env.project).and_then(|()| f(&tmp, source));
    let _ = std::fs::remove_dir_all(&tmp);
    res
}

fn clone_url(source: &str) -> String {
    if source.contains("://") || source.starts_with("git@") {
        source.to_string()
    } else {
        format!("https://github.com/{source}.git")
    }
}

fn clone_command(source: &str, dest: &Path) -> Cmd {
    let args = [
        "clone",
        "--depth",
        "1",
        &clone_url(source),
        &dest.to_string_lossy(),
    ]
    .map(String::from)
    .to_vec();
    ("git".into(), args)
}

fn unique_name() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("skillz-install-{}-{nanos}", std::process::id())
}

/// Hide `user:token@` credentials in URLs before echoing a command.
fn redact(arg: &str) -> String {
    match (arg.find("://"), arg.find('@')) {
        (Some(scheme), Some(at)) if at > scheme && !arg[scheme + 3..at].contains('/') => {
            format!("{}://***{}", &arg[..scheme], &arg[at..])
        }
        _ => arg.to_string(),
    }
}

pub(crate) fn run_checked(
    runner: &dyn CommandRunner,
    program: &str,
    args: &[String],
    cwd: &Path,
) -> anyhow::Result<()> {
    let shown = args.iter().map(|a| redact(a)).collect::<Vec<_>>().join(" ");
    eprintln!("$ {program} {shown}");
    let out = runner.run(program, args, cwd)?;
    anyhow::ensure!(
        out.success,
        "`{program} {shown}` failed: {}",
        redact(out.stderr.trim())
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::test_support::*;
    use std::cell::Cell;

    #[test]
    fn local_source_is_used_in_place_as_absolute_path() {
        let fx = project_with(skills_fixture);
        let runner = FakeRunner::new(|_| {});
        let mut got = None;
        with_source("repo", &fx.env.project, &fx.env, &runner, |tree, source| {
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
        with_source("repo", &fx.env.project, &env, &runner, |tree, _| {
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
            let res = with_source(source, &fx.env.project, &fx.env, &runner, |_, _| Ok(()));
            assert!(res.is_err());
        }
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn clone_failure_is_err_and_skips_install() {
        let fx = project_with(|_| {});
        let runner = FakeRunner::new(skills_fixture).failing("git");
        let called = Cell::new(false);
        let res = with_source("org/repo", &fx.env.project, &fx.env, &runner, |_, _| {
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
            &fx.env,
            &runner,
            |_, _| Ok(()),
        )
        .unwrap_err();
        assert!(!format!("{err:#}").contains("s3cret"));
    }

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
    fn redact_hides_url_credentials_only() {
        assert!(!redact("https://u:tok@github.com/o/r.git").contains("tok"));
        assert_eq!(
            redact("https://github.com/o/r.git"),
            "https://github.com/o/r.git"
        );
        assert_eq!(redact("git@github.com:o/r.git"), "git@github.com:o/r.git");
        assert_eq!(redact("https://github.com/a@b"), "https://github.com/a@b");
    }
}
