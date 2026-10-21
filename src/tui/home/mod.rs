//! The workspace home: registered projects on the left, profiles in the middle, and a detail
//! pane for whatever is selected. `state` is a pure machine (`Home::handle`) that only says what
//! the user asked for (`Act`); `cli::workspace` does it and shows the screen again.

mod ask;
mod model;
mod state;
#[cfg(test)]
mod tests;
mod view;

pub use model::{Act, Focus, Model, Pos, ProfileRow, ProjectRow};
pub use state::Home;

use super::terminal::event_loop;
use super::theme::Theme;

/// Show the home until the user asks for something. Errors when there is no terminal.
pub fn run(model: Model, pos: Pos, theme: &Theme) -> anyhow::Result<(Act, Pos)> {
    let mut home = Home::new(model, pos);
    let act = event_loop(&mut home, |h, f| h.draw(f, theme), |h, k| h.handle(k))?;
    Ok((act, home.pos()))
}
