//! Minimal terminal widgets: a multi-select list and a collapsible tree, both with `/` filter.
//! Each widget is a pure state machine (`State::handle(key)`) plus a thin render/event loop.

use std::io::IsTerminal;

use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

pub mod select;
pub mod tree_view;

/// Result of feeding a key to the `/` filter input.
enum FilterKey {
    Edited,
    Left,
    Other,
}

fn filter_key(filter: &mut String, code: KeyCode) -> FilterKey {
    match code {
        KeyCode::Char(c) => filter.push(c),
        KeyCode::Backspace => {
            filter.pop();
        }
        KeyCode::Esc => {
            filter.clear();
            return FilterKey::Left;
        }
        KeyCode::Enter => return FilterKey::Left,
        _ => return FilterKey::Other,
    }
    FilterKey::Edited
}

fn is_ctrl_c(key: KeyEvent) -> bool {
    key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)
}

fn step_cursor(cursor: usize, len: usize, up: bool) -> usize {
    match (up, len) {
        (_, 0) => 0,
        (true, _) => cursor.saturating_sub(1),
        (false, _) => (cursor + 1).min(len - 1),
    }
}

/// Draw `state`, feed key presses to `step` until it yields a result. Always restores the terminal.
fn event_loop<S, R>(
    state: &mut S,
    draw: impl Fn(&S, &mut Frame),
    step: impl Fn(&mut S, KeyEvent) -> Option<R>,
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

#[cfg(test)]
fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[cfg(test)]
fn keys(s: &str) -> Vec<KeyEvent> {
    s.chars().map(|c| key(KeyCode::Char(c))).collect()
}
