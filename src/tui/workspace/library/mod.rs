//! The Library tab: the catalog of skills, plugins, MCP servers, hooks and subagents that can
//! be installed or put in a profile, with adding, importing and removing. The rows are derived
//! from the library each time the workspace loads a new snapshot.

mod draw;
mod install;
mod keys;
mod rows;
#[cfg(test)]
mod tests;

use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

use self::install::InstallDialog;
use self::rows::{Row, build, shape};
use super::tab::Tab;
use crate::app::library::Held;
use crate::app::stale::Disk;
use crate::app::workspace::Act;
use crate::app::workspace::Model;
use crate::tui::keymap::Scope;
use crate::tui::theme::Theme;
use crate::tui::widgets::confirm::ConfirmDialog;
use crate::tui::widgets::input::InputField;
use crate::tui::widgets::outline::Outline;
use skillz_harness::Spec;
use skillz_harness::model::Agent;

/// Which popup, if any, has the keys.
#[derive(Debug, Clone, Default)]
enum Popup {
    #[default]
    None,
    Filter,
    Add(InputField),
    /// Asking whether to remove this entry from the library.
    Remove(ConfirmDialog, Spec),
    /// Choosing where and for which tools to install this entry.
    Install(InstallDialog, Spec),
}

pub struct LibraryTab {
    rows: Vec<Row>,
    outline: Outline,
    popup: Popup,
    /// The tools an install can be made for.
    agents: Vec<Agent>,
}

impl LibraryTab {
    pub fn new() -> Self {
        LibraryTab {
            rows: vec![],
            outline: Outline::new(vec![]),
            popup: Popup::None,
            agents: vec![],
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
        self.agents.clone_from(&model.agents);
        self.rows = build(
            &model.library,
            &Held::of(&model.installed, &model.global),
            &Disk,
        );
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
