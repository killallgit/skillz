use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::Context;

/// Result of an external command; stderr is captured for error messages.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Output {
    pub success: bool,
    pub stderr: String,
}

/// Runs external programs (`git`, `npx`, `claude`). Injected so tests never spawn processes.
pub trait CommandRunner {
    fn run(&self, program: &str, args: &[String], cwd: &Path) -> anyhow::Result<Output>;

    /// Like `run`, but the child's stdout and stdin are not the terminal's: its progress chatter
    /// would otherwise print over our own status line. Runners that cannot tell just `run`.
    fn run_quiet(&self, program: &str, args: &[String], cwd: &Path) -> anyhow::Result<Output> {
        self.run(program, args, cwd)
    }

    /// Run a program the user works in (an agent), with the whole terminal attached. `true` when
    /// it exited successfully.
    fn launch(&self, program: &str, args: &[String], cwd: &Path) -> anyhow::Result<bool> {
        Ok(self.run(program, args, cwd)?.success)
    }

    /// The same runner with these environment variables set on every child, when it can set any.
    fn with_envs(&self, _envs: Vec<(&'static str, PathBuf)>) -> Option<Box<dyn CommandRunner>> {
        None
    }

    /// Run and return stdout; a non-zero exit is an error. Runners that cannot capture say so.
    fn capture(&self, program: &str, _args: &[String], _cwd: &Path) -> anyhow::Result<String> {
        anyhow::bail!("cannot capture the output of `{program}` here")
    }
}

/// For callers whose plan runs no commands: any command is an error.
pub struct NoCommands;

impl CommandRunner for NoCommands {
    fn run(&self, program: &str, _args: &[String], _cwd: &Path) -> anyhow::Result<Output> {
        anyhow::bail!("no commands allowed here, got `{program}`")
    }
}

/// Spawns real processes in `cwd` with stdin and stdout inherited (npx and claude print
/// prompts and errors to stdout) and stderr captured for the error message. `envs` are set on
/// every child, so a config dir skillz was pointed at (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`)
/// reaches `claude`, `codex` and `npx` as well, whatever the parent process holds.
#[derive(Default)]
pub struct SystemRunner {
    pub envs: Vec<(&'static str, PathBuf)>,
}

impl CommandRunner for SystemRunner {
    fn launch(&self, program: &str, args: &[String], cwd: &Path) -> anyhow::Result<bool> {
        let status = Command::new(program)
            .args(args)
            .envs(self.envs.iter().map(|(k, v)| (k, v)))
            .current_dir(cwd)
            .status()
            .with_context(|| format!("failed to run `{program}` in {}", cwd.display()))?;
        Ok(status.success())
    }

    fn with_envs(&self, envs: Vec<(&'static str, PathBuf)>) -> Option<Box<dyn CommandRunner>> {
        Some(Box::new(SystemRunner { envs }))
    }

    fn run(&self, program: &str, args: &[String], cwd: &Path) -> anyhow::Result<Output> {
        let out = Command::new(program)
            .args(args)
            .envs(self.envs.iter().map(|(k, v)| (k, v)))
            .current_dir(cwd)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .output()
            .with_context(|| format!("failed to run `{program}` in {}", cwd.display()))?;
        Ok(Output {
            success: out.status.success(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }

    fn run_quiet(&self, program: &str, args: &[String], cwd: &Path) -> anyhow::Result<Output> {
        let out = Command::new(program)
            .args(args)
            .envs(self.envs.iter().map(|(k, v)| (k, v)))
            .current_dir(cwd)
            .stdin(Stdio::null())
            .output()
            .with_context(|| format!("failed to run `{program}` in {}", cwd.display()))?;
        // Some tools report failures on stdout, which we are not showing.
        let said = if out.stderr.is_empty() {
            &out.stdout
        } else {
            &out.stderr
        };
        Ok(Output {
            success: out.status.success(),
            stderr: String::from_utf8_lossy(said).into_owned(),
        })
    }

    fn capture(&self, program: &str, args: &[String], cwd: &Path) -> anyhow::Result<String> {
        let out = Command::new(program)
            .args(args)
            .envs(self.envs.iter().map(|(k, v)| (k, v)))
            .current_dir(cwd)
            .stdin(Stdio::null())
            .output()
            .with_context(|| format!("failed to run `{program}` in {}", cwd.display()))?;
        anyhow::ensure!(
            out.status.success(),
            "`{program}` failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sh(script: &str) -> anyhow::Result<Output> {
        let dir = tempfile::tempdir().unwrap();
        SystemRunner::default().run("sh", &["-c".into(), script.into()], dir.path())
    }

    #[test]
    fn captures_stderr_and_success() {
        let out = sh("printf err >&2").unwrap();
        assert_eq!(
            out,
            Output {
                success: true,
                stderr: "err".into()
            }
        );
    }

    #[test]
    fn configured_envs_reach_the_child() {
        let dir = tempfile::tempdir().unwrap();
        let runner = SystemRunner {
            envs: vec![("SKILLZ_TEST_DIR", dir.path().to_path_buf())],
        };
        let args = ["-c".into(), "printf %s \"$SKILLZ_TEST_DIR\"".into()];
        let out = runner.capture("sh", &args, dir.path()).unwrap();
        assert_eq!(out, dir.path().to_string_lossy());
    }

    #[test]
    fn quiet_runs_swallow_stdout_but_keep_stderr() {
        let dir = tempfile::tempdir().unwrap();
        let args = ["-c".into(), "echo noise; echo oops >&2; exit 2".into()];
        let out = SystemRunner::default()
            .run_quiet("sh", &args, dir.path())
            .unwrap();
        assert_eq!((out.success, out.stderr.trim()), (false, "oops"));
        let args = ["-c".into(), "echo only-stdout; exit 1".into()];
        let out = SystemRunner::default()
            .run_quiet("sh", &args, dir.path())
            .unwrap();
        assert_eq!(out.stderr.trim(), "only-stdout");
    }

    #[test]
    fn nonzero_exit_is_not_success() {
        assert!(!sh("exit 3").unwrap().success);
    }

    #[test]
    fn capture_returns_stdout_and_fails_on_nonzero_exit() {
        let dir = tempfile::tempdir().unwrap();
        let sh = |script: &str| {
            SystemRunner::default().capture("sh", &["-c".into(), script.into()], dir.path())
        };
        assert_eq!(sh("printf hi").unwrap(), "hi");
        assert!(sh("exit 2").is_err());
        assert!(NoCommands.capture("x", &[], dir.path()).is_err());
    }

    #[test]
    fn runs_in_cwd() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("marker"), "").unwrap();
        let args = ["-c".into(), "test -f marker".into()];
        assert!(
            SystemRunner::default()
                .run("sh", &args, dir.path())
                .unwrap()
                .success
        );
    }

    #[test]
    fn missing_program_is_err() {
        let dir = tempfile::tempdir().unwrap();
        let res = SystemRunner::default().run("skillz-definitely-not-a-program", &[], dir.path());
        assert!(res.is_err());
    }
}
