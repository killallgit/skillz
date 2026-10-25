//! What the workspace and the plain commands share for changing things: one project's stores
//! and the changes to the project, library and profiles, each with the plan reviewed first.

mod change;
mod console;
mod library_change;
mod notice;
mod perform;
mod plan_view;
mod profiles;
mod selection;
mod source_install;
mod updates;

use std::io::Write;
use std::path::PathBuf;

use super::deps::Deps;
use crate::app::{profile, store};
use skillz_harness::env::Env;

pub(super) use change::{Change, change};
pub(super) use console::Console;
pub(super) use notice::Notice;
pub(super) use selection::Selection;

/// The project, stores and prompts one session works with.
pub(super) struct Session<'a, 'd> {
    deps: &'a Deps<'d>,
    profiles: profile::Store,
    store: store::Store,
    project: PathBuf,
}

impl<'a, 'd> Session<'a, 'd> {
    pub(super) fn open(deps: &'a Deps<'d>, env: &Env) -> Self {
        Session {
            deps,
            profiles: profile::Store::in_env(env),
            store: store::Store::in_env(env),
            project: env.project.clone(),
        }
    }

    pub(super) fn deps(&self) -> &Deps<'d> {
        self.deps
    }

    pub(super) fn project(&self) -> &std::path::Path {
        &self.project
    }

    pub(super) fn profiles(&self) -> &profile::Store {
        &self.profiles
    }

    pub(super) fn library(&self) -> &store::Store {
        &self.store
    }
}

/// Run `act` on a session whose prompts are lines on `out`; with `dry_run` its plans are shown
/// and never applied.
pub(super) fn with_session<T>(
    deps: &Deps,
    env: &Env,
    out: &mut dyn Write,
    dry_run: bool,
    act: impl FnOnce(&Session, &Console) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let console = Console::new(deps.prompter, out, dry_run);
    let with_console = deps.with_prompter(&console);
    act(&Session::open(&with_console, env), &console)
}
