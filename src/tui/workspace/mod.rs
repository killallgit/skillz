//! The workspace for one directory: tabs for the project, the library and the profiles. Each tab
//! is a `Tab` in its own module; the `Workspace` shell owns the tab bar, key bar, help and
//! toasts, and hands every tab the same `Model`. Keys become an `Act` that `cli::workspace`
//! performs before the screen is shown again.

mod act;
mod checks;
mod library;
mod model;
mod profiles;
mod project;
mod shell;
mod tab;

pub use act::Act;
pub use model::Model;
pub use shell::Workspace;
pub use tab::Tab;

use super::closure::event_loop;
use super::shell::Ui;

/// Show the workspace until the user asks for something. Errors when there is no terminal.
pub fn run(workspace: &mut Workspace, ui: &Ui) -> anyhow::Result<Act> {
    let act = event_loop(
        ui,
        workspace,
        |w, frame, theme| w.draw(frame, theme),
        |w, key| w.handle(key),
    )?;
    Ok(act.unwrap_or(Act::Quit))
}
