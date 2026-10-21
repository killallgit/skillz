//! Helpers shared by widget tests.

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Draw one frame on an in-memory terminal and hand back its cells.
pub fn render_to_buffer(width: u16, height: u16, draw: impl FnOnce(&mut ratatui::Frame)) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(draw).unwrap();
    terminal.backend().buffer().clone()
}

pub fn row_text(buffer: &Buffer, y: u16) -> String {
    (0..buffer.area.width)
        .map(|x| buffer[(x, y)].symbol())
        .collect()
}

pub fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

pub fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

/// True when the cell shows everything `style` sets: its colours and its modifiers.
pub fn has_style(cell: &ratatui::buffer::Cell, style: ratatui::style::Style) -> bool {
    style.fg.is_none_or(|c| cell.fg == c)
        && style.bg.is_none_or(|c| cell.bg == c)
        && cell.modifier.contains(style.add_modifier)
}

/// One key press per character of `s`.
pub fn keys(s: &str) -> Vec<KeyEvent> {
    s.chars().map(|c| key(KeyCode::Char(c))).collect()
}
