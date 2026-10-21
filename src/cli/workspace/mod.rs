//! The workspace for one directory: what it has installed, the library and the profiles, with
//! installs, removals and profile applies shown as a plan first, as for `use`.

mod change;
mod model;
mod notice;
mod perform;
mod plan_view;

use std::path::{Path, PathBuf};

use super::deps::Deps;
use super::scope::resolve_project;
use crate::library;
use crate::profile;
use crate::tui::workspace::{Act, Workspace};
use model::snapshot;
use notice::Notice;

/// The project, stores and prompts one session works with.
struct Session<'a, 'd> {
    deps: &'a Deps<'d>,
    profiles: profile::Store,
    library: library::Store,
    project: PathBuf,
}

/// Open the workspace on `path` (default: the current directory). An act that fails shows its
/// error as a toast and the workspace stays open. The terminal is entered once and kept until
/// the workspace closes.
pub fn workspace(deps: &Deps, path: Option<&Path>) -> anyhow::Result<()> {
    let _held = deps.prompter.hold();
    let env = resolve_project(deps, path)?;
    let session = Session {
        deps,
        profiles: profile::Store::in_env(&env),
        library: library::Store::in_env(&env),
        project: env.project,
    };
    let mut screen = Workspace::standard();
    let mut notice = None;
    loop {
        screen.load(snapshot(&session, notice.take())?);
        let act = deps.prompter.workspace(&mut screen)?;
        if act == Act::Quit {
            return Ok(());
        }
        notice = session
            .perform(act)
            .unwrap_or_else(|e| Some(Notice::Failed(format!("{e:#}"))));
    }
}
