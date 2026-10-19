use std::path::Path;

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

/// Spawns real processes, inheriting stdio so the user sees progress.
pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, program: &str, args: &[String], cwd: &Path) -> anyhow::Result<Output> {
        let _ = (program, args, cwd);
        todo!("lane: install")
    }
}
