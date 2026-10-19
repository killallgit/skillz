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
