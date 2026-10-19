use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::Context;

/// Captured result of an external command.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Output {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Runs external programs (`git`, `npx`, `claude`). Injected so tests never spawn processes.
pub trait CommandRunner {
    fn run(&self, program: &str, args: &[String], cwd: &Path) -> anyhow::Result<Output>;
}

/// Spawns real processes in `cwd` with stdin inherited (for credential prompts) and
/// stdout/stderr captured so callers can parse or report them.
pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, program: &str, args: &[String], cwd: &Path) -> anyhow::Result<Output> {
        let out = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::inherit())
            .output()
            .with_context(|| format!("failed to run `{program}` in {}", cwd.display()))?;
        Ok(Output {
            success: out.status.success(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
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
    fn captures_stdout_stderr_and_success() {
        let out = sh("printf out; printf err >&2").unwrap();
        assert!(out.success);
        assert_eq!(out.stdout, "out");
        assert_eq!(out.stderr, "err");
    }

    #[test]
    fn nonzero_exit_is_not_success() {
        assert!(!sh("exit 3").unwrap().success);
    }

    #[test]
    fn runs_in_cwd() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("marker"), "").unwrap();
        let out = SystemRunner.run("ls", &[], dir.path()).unwrap();
        assert_eq!(out.stdout.trim(), "marker");
    }

    #[test]
    fn missing_program_is_err() {
        let dir = tempfile::tempdir().unwrap();
        let res = SystemRunner.run("skillz-definitely-not-a-program", &[], dir.path());
        assert!(res.is_err());
    }
}
