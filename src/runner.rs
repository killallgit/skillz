use std::path::Path;
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
/// prompts and errors to stdout) and stderr captured for the error message.
pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, program: &str, args: &[String], cwd: &Path) -> anyhow::Result<Output> {
        let out = Command::new(program)
            .args(args)
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

    fn capture(&self, program: &str, args: &[String], cwd: &Path) -> anyhow::Result<String> {
        let out = Command::new(program)
            .args(args)
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
        SystemRunner.run("sh", &["-c".into(), script.into()], dir.path())
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
    fn nonzero_exit_is_not_success() {
        assert!(!sh("exit 3").unwrap().success);
    }

    #[test]
    fn capture_returns_stdout_and_fails_on_nonzero_exit() {
        let dir = tempfile::tempdir().unwrap();
        let sh =
            |script: &str| SystemRunner.capture("sh", &["-c".into(), script.into()], dir.path());
        assert_eq!(sh("printf hi").unwrap(), "hi");
        assert!(sh("exit 2").is_err());
        assert!(NoCommands.capture("x", &[], dir.path()).is_err());
    }

    #[test]
    fn runs_in_cwd() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("marker"), "").unwrap();
        let args = ["-c".into(), "test -f marker".into()];
        assert!(SystemRunner.run("sh", &args, dir.path()).unwrap().success);
    }

    #[test]
    fn missing_program_is_err() {
        let dir = tempfile::tempdir().unwrap();
        let res = SystemRunner.run("skillz-definitely-not-a-program", &[], dir.path());
        assert!(res.is_err());
    }
}
