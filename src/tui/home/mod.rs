//! The workspace home: registered projects on the left, profiles in the middle, and a detail
//! pane for whatever is selected. `state` is a pure machine (`Home::handle`) that only says what
//! the user asked for (`Act`); `cli::workspace` does it and shows the screen again.

mod ask;
mod detail;
mod header;
mod layout;
mod model;
mod mouse;
#[cfg(test)]
mod mouse_tests;
mod pane;
mod popups;
mod rows;
mod screen;
#[cfg(test)]
mod snapshots;
mod state;
#[cfg(test)]
mod tests;
mod view;
mod welcome;

pub use model::{
    Act, Current, DetailLine, EntryKind, Focus, Model, Pos, ProfileRow, ProjectRow,
};
pub use screen::HomeScreen;
pub use state::Home;

use super::app::App;
use super::shell::Ui;

/// Show the home until the user asks for something. Errors when there is no terminal.
pub fn run(model: Model, pos: Pos, ui: &Ui) -> anyhow::Result<(Act, Pos)> {
    let mut home = Home::new(model, pos);
    let act = ui.run(&mut App::new(HomeScreen(&mut home)))?;
    Ok((act.unwrap_or(Act::Quit), home.pos()))
}
