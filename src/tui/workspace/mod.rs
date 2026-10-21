//! The workspace for one directory: tabs for the project, the library and the profiles. Each tab
//! is a `Tab` in its own module; the `Workspace` shell owns the tab bar, key bar, help and
//! toasts, and hands every tab the same `Model`. Keys become an `Act` that `cli::workspace`
//! performs before the screen is shown again.

mod act;
mod checks;
mod installed;
mod library;
mod model;
mod profiles;
mod shell;
mod tab;

pub use act::Act;
pub use model::Model;
pub use shell::Workspace;
pub use tab::Tab;

use std::sync::{Arc, Mutex, PoisonError};

use super::closure::event_loop;
use super::shell::Ui;

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
