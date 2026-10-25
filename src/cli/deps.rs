use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use crate::harness::agents::Harness;
use crate::harness::runner::CommandRunner;
use crate::harness::update::check::UpdateCheck;
use crate::prompt::Prompter;
use crate::sync::Scanner;

/// External effects the commands need; real ones in `main`, fakes in tests.
pub struct Deps<'a> {
    pub home: PathBuf,
    /// `CLAUDE_CONFIG_DIR`, when set: the only Claude config skillz touches.
    pub claude_config: Option<PathBuf>,
    /// `CODEX_HOME`, when set: the only Codex config skillz touches.
    pub codex_home: Option<PathBuf>,
    pub cwd: PathBuf,
    pub adapters: Rc<[Box<dyn Harness>]>,
    pub runner: &'a dyn CommandRunner,
    pub prompter: &'a dyn Prompter,
    pub interactive: bool,
    /// Finds out whether installed items have a newer version.
    pub checker: Arc<dyn UpdateCheck>,
    /// Finds what is installed so the library can be synced in the background; `None` turns
    /// that off.
    pub scanner: Option<Arc<dyn Scanner>>,
}

#[cfg(test)]
impl<'a> Deps<'a> {
    /// Deps rooted in `dir` (home and cwd), with the given adapters and fakes.
    pub(super) fn for_test(
        dir: &std::path::Path,
        adapters: Vec<Box<dyn Harness>>,
        (runner, prompter): (&'a dyn CommandRunner, &'a dyn Prompter),
    ) -> Deps<'a> {
        Deps {
            home: dir.to_path_buf(),
            claude_config: None,
            codex_home: None,
            cwd: dir.to_path_buf(),
            adapters: adapters.into(),
            runner,
            prompter,
            interactive: false,
            checker: Arc::new(crate::harness::update::check::NoCheck),
            scanner: None,
        }
    }
}

impl<'a> Deps<'a> {
    /// The same effects asking `prompter` instead.
    pub(super) fn with_prompter<'b>(&'b self, prompter: &'b dyn Prompter) -> Deps<'b> {
        Deps {
            home: self.home.clone(),
            claude_config: self.claude_config.clone(),
            codex_home: self.codex_home.clone(),
            cwd: self.cwd.clone(),
            adapters: Rc::clone(&self.adapters),
            runner: self.runner,
            prompter,
            interactive: self.interactive,
            checker: Arc::clone(&self.checker),
            scanner: self.scanner.clone(),
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
            adapters: Rc::clone(&self.adapters),
            runner,
            prompter: self.prompter,
            interactive: self.interactive,
            checker: Arc::clone(&self.checker),
            scanner: self.scanner.clone(),
        }
    }
}
