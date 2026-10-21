use std::path::PathBuf;

use crate::agents::AgentAdapter;
use crate::prompt::Prompter;
use crate::runner::CommandRunner;

/// External effects the commands need; real ones in `main`, fakes in tests.
pub struct Deps<'a> {
    pub home: PathBuf,
    /// `CLAUDE_CONFIG_DIR`, when set: the only Claude config skillz touches.
    pub claude_config: Option<PathBuf>,
    /// `CODEX_HOME`, when set: the only Codex config skillz touches.
    pub codex_home: Option<PathBuf>,
    pub cwd: PathBuf,
    pub adapters: Vec<Box<dyn AgentAdapter>>,
    pub runner: &'a dyn CommandRunner,
    pub prompter: &'a dyn Prompter,
    pub interactive: bool,
}

impl<'a> Deps<'a> {
    /// The same effects asking `prompter` instead.
    pub(super) fn with_prompter<'b>(&'b self, prompter: &'b dyn Prompter) -> Deps<'b> {
        Deps {
            home: self.home.clone(),
            claude_config: self.claude_config.clone(),
            codex_home: self.codex_home.clone(),
            cwd: self.cwd.clone(),
            adapters: crate::agents::registry(),
            runner: self.runner,
            prompter,
            interactive: self.interactive,
        }
    }

    /// The same effects aimed at other config dirs, running commands with `runner`.
    pub(super) fn redirected(
        &self,
        claude_config: Option<PathBuf>,
        codex_home: Option<PathBuf>,
        runner: &'a dyn CommandRunner,
    ) -> Deps<'a> {
        Deps {
            home: self.home.clone(),
            claude_config,
            codex_home,
            cwd: self.cwd.clone(),
            adapters: crate::agents::registry(),
            runner,
            prompter: self.prompter,
            interactive: self.interactive,
        }
    }
}
