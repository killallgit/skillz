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
