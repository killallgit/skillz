//! The terminal list: one collapsible, markable tree with `/` filter, used for browsing and
//! for every picker. A pure state machine (`State::handle(key)`) plus a thin render/event loop.

use std::io::IsTerminal;

use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{List, ListState, Paragraph};

pub mod home;
pub mod theme;
pub mod tree_view;
pub mod widgets;

/// Whether a row is chosen: all of it, only part of it (a group with some items marked), or not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    No,
    Part,
    All,
}

impl Mark {
    /// Two columns: an `X` (or `-` for part) and a space, so rows line up.
    fn column(self) -> &'static str {
        match self {
            Mark::No => "  ",
            Mark::Part => "- ",
            Mark::All => "X ",
        }
    }
}

/// The list both pickers share: a title line, borderless rows with the cursor row reversed,
/// and a footer. Returns the rows' area, for overlays.
fn draw_list(
    frame: &mut Frame,
    title: &str,
    header: &str,
    rows: Vec<String>,
    cursor: usize,
    footer: String,
) -> Rect {
    let [head, cols, body, foot] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    frame.render_widget(
        Paragraph::new(title.to_string()).style(Style::new().add_modifier(Modifier::BOLD)),
        head,
    );
    frame.render_widget(
        Paragraph::new(header.to_string()).style(Style::new().add_modifier(Modifier::DIM)),
        cols,
    );
    let list = List::new(rows).highlight_style(Style::new().add_modifier(Modifier::REVERSED));
    frame.render_stateful_widget(
        list,
        body,
        &mut ListState::default().with_selected(Some(cursor)),
    );
    frame.render_widget(Paragraph::new(footer), foot);
    body
}

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

#[cfg(test)]
mod mark_tests {
    use super::*;

    #[test]
    fn mark_columns_have_the_same_width() {
        let widths: Vec<usize> = [Mark::No, Mark::Part, Mark::All]
            .iter()
            .map(|m| m.column().chars().count())
            .collect();
        assert!(widths.iter().all(|w| *w == widths[0]));
        assert!(Mark::All.column().starts_with('X'));
    }
}
