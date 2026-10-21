//! The Library tab: the catalog of skills, plugins, MCP servers, hooks and subagents that can
//! be installed or put in a profile, with adding, importing and removing. The rows are derived
//! from the library each time the workspace loads a new snapshot.

mod detail;
mod draw;
mod keys;
mod presence;
mod rows;
#[cfg(test)]
mod tests;

use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

use self::presence::{Presence, presences};
use self::rows::{Row, build, shape};
use super::act::Act;
use super::model::Model;
use super::tab::Tab;
use crate::profile::Entry;
use crate::tui::keymap::Scope;
use crate::tui::theme::Theme;
use crate::tui::widgets::confirm::ConfirmDialog;
use crate::tui::widgets::input::InputField;
use crate::tui::widgets::outline::Outline;

/// Which popup, if any, has the keys.
#[derive(Debug, Clone, Default)]
enum Popup {
    #[default]
    None,
    Filter,
    Add(InputField),
    /// Asking whether to remove this entry from the library.
    Remove(ConfirmDialog, Entry),
}

pub struct LibraryTab {
    rows: Vec<Row>,
    presence: Vec<Presence>,
    outline: Outline,
    popup: Popup,
}

impl LibraryTab {
    pub fn new() -> Self {
        LibraryTab {
            rows: vec![],
            presence: vec![],
            outline: Outline::new(vec![]),
            popup: Popup::None,
        }
    }
}

impl Tab for LibraryTab {
    fn title(&self) -> &'static str {
        "Library"
    }

    fn scopes(&self) -> &'static [Scope] {
        &[Scope::Library]
    }

    fn load(&mut self, model: &Model) {
        self.rows = build(&model.library);
        self.presence = presences(&self.rows, model);
        self.outline.reshape(shape(&self.rows));
    }

    fn handle(&mut self, key: KeyEvent) -> Option<Act> {
        self.handle_key(key)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        self.draw_body(frame, area, theme);
    }

    fn captures_keys(&self) -> bool {
        !matches!(self.popup, Popup::None)
    }

    fn prompt(&self) -> Option<&InputField> {
        matches!(self.popup, Popup::Filter).then(|| self.outline.field())
    }
}
