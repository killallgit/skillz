//! The workspace for one directory: the plugins list of the project or of the user. The list
//! (`PluginsTab`) is in its own module; the `Workspace` shell owns help, toasts and the change
//! under way, and hands the list the `Model`. Keys become an `Act` that `cli::workspace`
//! performs before the screen is shown again.

mod plugins;
mod run;
mod shell;
mod syncs;
mod tools;

pub use shell::Workspace;

use std::sync::{Arc, Mutex, PoisonError};

use super::closure::event_loop;
use super::shell::Ui;
use crate::app::workspace::Act;

/// The workspace, shared with whatever redraws it while a change runs.
pub type Shared = Arc<Mutex<Workspace>>;

/// Show the workspace until the user asks for something. Errors when there is no terminal.
pub fn run(shared: &Shared, ui: &Ui) -> anyhow::Result<Act> {
    let mut workspace = shared.lock().unwrap_or_else(PoisonError::into_inner);
    let act = event_loop(
        ui,
        &mut *workspace,
        |w, frame, theme| w.draw(frame, theme),
        |w, key| w.handle(key),
    )?;
    Ok(act.unwrap_or(Act::Quit))
}
