//! Key predicates shared by every screen.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn is_ctrl_c(key: KeyEvent) -> bool {
    key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)
}

#[cfg(test)]
pub fn keys(s: &str) -> Vec<KeyEvent> {
    s.chars()
        .map(|c| super::widgets::testing::key(KeyCode::Char(c)))
        .collect()
}
