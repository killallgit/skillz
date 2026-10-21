//! The command line: argument definitions, scope resolution and one module per command group.

mod args;
mod deps;
mod isolate;
mod list;
mod notice;
mod profile;
mod project;
mod prune;
mod removal;
mod run;
mod scope;
mod screen;
mod switch;
mod use_profile;
mod workspace;

use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;

use args::wants_auto_yes;
pub use args::{
    Cli, Command, Filter, InstallArgs, ListArgs, ProfileCommand, ProjectCommand, PruneArgs,
    ResetArgs, UninstallArgs, UseArgs,
};
pub use deps::Deps;
pub use run::run;
pub use workspace::workspace;

use crate::agents;
use crate::prompt::{AssumeYes, Prompter};
use crate::runner::SystemRunner;
use crate::tui::prompter::TuiPrompter;
use crate::tui::theme::Theme;

/// Returns the process exit code.
pub fn main() -> anyhow::Result<i32> {
    let cli = Cli::parse();
    let theme = Theme::detect(&|name| std::env::var(name).ok());
    let tui = TuiPrompter::new(theme, std::env::var_os("SKILLZ_NO_MOUSE").is_none());
    let prompter: &dyn Prompter = if cli.command.as_ref().is_some_and(wants_auto_yes) {
        &AssumeYes
    } else {
        &tui
    };
    let claude_config = redirected_dir("CLAUDE_CONFIG_DIR");
    let codex_home = redirected_dir("CODEX_HOME");
    let runner = child_runner(&claude_config, &codex_home);
    let deps = Deps {
        home: std::env::home_dir().context("cannot find home directory")?,
        claude_config,
        codex_home,
        cwd: std::env::current_dir()?,
        adapters: agents::registry().into(),
        runner: &runner,
        prompter,
        // Pickers and the browser read keys from stdin and draw on stdout.
        interactive: std::io::stdin().is_terminal() && std::io::stdout().is_terminal(),
    };
    match cli.command {
        Some(command) => run(command, &deps, &mut std::io::stdout()),
        None => workspace(&deps).map(|()| 0),
    }
}

/// The directory an agent's config is redirected to, when the variable is set.
fn redirected_dir(variable: &str) -> Option<PathBuf> {
    std::env::var_os(variable)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Runs child commands with the same redirects, so native tools act on the same directories.
fn child_runner(claude_config: &Option<PathBuf>, codex_home: &Option<PathBuf>) -> SystemRunner {
    let redirects = [
        ("CLAUDE_CONFIG_DIR", claude_config),
        ("CODEX_HOME", codex_home),
    ];
    SystemRunner {
        envs: redirects
            .into_iter()
            .filter_map(|(name, dir)| Some((name, dir.clone()?)))
            .collect(),
    }
}
