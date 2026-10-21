//! The blocking draw/read loop every screen runs in.

use std::io::IsTerminal;

use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyEvent, KeyEventKind};

/// Draw `state`, feed key presses to `step` until it yields a result. Always restores the terminal.
pub fn event_loop<S, R>(
    state: &mut S,
    mut draw: impl FnMut(&mut S, &mut Frame),
    mut step: impl FnMut(&mut S, KeyEvent) -> Option<R>,
) -> anyhow::Result<R> {
    if !(std::io::stdin().is_terminal() && std::io::stdout().is_terminal()) {
        anyhow::bail!("interactive mode needs a terminal; pass --yes to run non-interactively");
    }
    let mut terminal = match ratatui::try_init() {
        Ok(t) => t,
        Err(e) => {
            ratatui::restore();
            return Err(e.into());
        }
    };
    let result = (|| loop {
        terminal.draw(|f| draw(state, f))?;
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && let Some(r) = step(state, key)
        {
            return Ok(r);
        }
    })();
    ratatui::restore();
    result
}
