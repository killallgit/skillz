//! The command line: argument definitions, scope resolution and one module per command group.

mod args;
mod deps;
mod isolate;
mod library;
mod library_args;
mod list;
mod pick;
mod profile;
mod prune;
mod recording;
mod removal;
mod run;
mod scope;
mod session;
mod sync;
mod update;
mod use_profile;
mod workspace;

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;
use clap::Parser;

use args::wants_auto_yes;
pub use args::{
    Cli, Command, Filter, InstallArgs, ListArgs, ProfileCommand, PruneArgs, ResetArgs, SyncArgs,
    UninstallArgs, UpdateArgs, UseArgs,
};
pub use deps::Deps;
pub use library_args::{
    AddArgs, KindFlags, LibraryCommand, LibraryInstallArgs, LibraryListArgs, LibraryRemoveArgs,
};
pub use run::run;
pub use workspace::workspace;

use crate::agents;
use crate::prompt::{AssumeYes, Prompter};
use crate::runner::SystemRunner;
use crate::sync::{InstalledScanner, Scanner};
use crate::tui::prompter::TuiPrompter;
use crate::tui::theme::Theme;
use crate::update::check::{ClaudeMarketplaces, NoCheck, UpdateCheck};

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
    let home = std::env::home_dir().context("cannot find home directory")?;
    let checker = update_checker(&home, &claude_config);
    let deps = Deps {
        home,
        claude_config,
        codex_home,
        cwd: std::env::current_dir()?,
        adapters: agents::registry().into(),
        runner: &runner,
        prompter,
        // Pickers and the browser read keys from stdin and draw on stdout.
        interactive: std::io::stdin().is_terminal() && std::io::stdout().is_terminal(),
        checker,
        scanner: background_scanner(),
    };
    match cli.command {
        Some(command) => run(command, &deps, &mut std::io::stdout()),
        None => workspace(&deps, cli.path.as_deref()).map(|()| 0),
    }
}

/// The startup scan that fills the library, unless `SKILLZ_NO_SYNC` is set.
fn background_scanner() -> Option<Arc<dyn Scanner>> {
    std::env::var_os("SKILLZ_NO_SYNC")
        .is_none()
        .then(|| Arc::new(InstalledScanner) as Arc<dyn Scanner>)
}

/// The network check for newer versions, unless `SKILLZ_NO_UPDATE_CHECK` is set.
fn update_checker(home: &Path, claude_config: &Option<PathBuf>) -> Arc<dyn UpdateCheck> {
    if std::env::var_os("SKILLZ_NO_UPDATE_CHECK").is_some() {
        return Arc::new(NoCheck);
    }
    let claude = claude_config
        .clone()
        .unwrap_or_else(|| home.join(".claude"));
    Arc::new(ClaudeMarketplaces {
        runner: SystemRunner {
            envs: vec![("GIT_TERMINAL_PROMPT", PathBuf::from("0"))],
        },
        clones: claude.join("plugins/marketplaces"),
    })
}

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
