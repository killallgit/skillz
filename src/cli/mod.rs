//! The command line: argument definitions, scope resolution and one module per command group.

mod args;
mod deps;
mod list;
mod profile;
mod project;
mod prune;
mod removal;
mod run;
mod scope;
mod use_profile;
mod workspace;

use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;

use args::wants_auto_yes;
pub use args::{Cli, Command, Filter, ProfileCommand, ProjectCommand};
pub use deps::Deps;
pub use run::run;
pub use workspace::workspace;

use crate::agents;
use crate::prompt::{AssumeYes, Prompter, TuiPrompter};
use crate::runner::SystemRunner;

pub fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let prompter: &dyn Prompter = if cli.command.as_ref().is_some_and(wants_auto_yes) {
        &AssumeYes
    } else {
        &TuiPrompter
    };
    let dir_var = |name: &str| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let claude_config = dir_var("CLAUDE_CONFIG_DIR");
    let codex_home = dir_var("CODEX_HOME");
    let runner = SystemRunner {
        envs: [
            ("CLAUDE_CONFIG_DIR", &claude_config),
            ("CODEX_HOME", &codex_home),
        ]
        .into_iter()
        .filter_map(|(k, v)| Some((k, v.clone()?)))
        .collect(),
    };
    let deps = Deps {
        home: std::env::home_dir().context("cannot find home directory")?,
        claude_config,
        codex_home,
        cwd: std::env::current_dir()?,
        adapters: agents::registry(),
        runner: &runner,
        prompter,
        // Pickers and the browser read keys from stdin and draw on stdout.
        interactive: std::io::stdin().is_terminal() && std::io::stdout().is_terminal(),
    };
    match cli.command {
        Some(command) => run(command, &deps, &mut std::io::stdout()),
        None => workspace(&deps, &mut std::io::stdout()),
    }
}
