//! Keys for the tree: `TreeState::handle` turns a key into a state change or an `Outcome`.

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::menu::{Menu, Pick};
use super::model::{Overlay, TreeState};
use crate::prompt::{Choice, Picked};
use crate::tui::keys::is_ctrl_c;
use crate::tui::widgets::input::Outcome as Edit;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Continue,
    Quit,
    Accept(Choice, Picked),
}

impl TreeState {
    pub fn handle(&mut self, key: KeyEvent) -> Outcome {
        if is_ctrl_c(key) {
            return Outcome::Quit;
        }
        match std::mem::take(&mut self.overlay) {
            Overlay::None => self.handle_browsing(key),
            Overlay::Filter => self.handle_filter(key),
            Overlay::Menu(menu) => self.handle_menu(menu, key),
            Overlay::Info(id) => self.handle_info(id, key),
            Overlay::Help => Outcome::Continue,
        }
    }

    fn handle_info(&mut self, id: usize, key: KeyEvent) -> Outcome {
        let closes = matches!(
            key.code,
            KeyCode::Esc | KeyCode::Char('q' | 'i') | KeyCode::Enter
        );
        if !closes {
            self.overlay = Overlay::Info(id);
        }
        Outcome::Continue
    }

    fn handle_browsing(&mut self, key: KeyEvent) -> Outcome {
        match key.code {
            KeyCode::Char('/') => self.overlay = Overlay::Filter,
            KeyCode::Char('?') => self.overlay = Overlay::Help,
            KeyCode::Char('x') | KeyCode::Tab => self.toggle_mark(),
            KeyCode::Char('i') => self.open_info(),
            KeyCode::Enter => return self.enter(),
            KeyCode::Esc if !self.filter.value().is_empty() => self.clear_filter(),
            KeyCode::Esc | KeyCode::Char('q') => return Outcome::Quit,
            _ => self.navigate(key),
        }
        Outcome::Continue
    }

    fn open_info(&mut self) {
        let with_info = self.current().filter(|&id| !self.rows[id].info.is_empty());
        if let Some(id) = with_info {
            self.overlay = Overlay::Info(id);
        }
    }

    fn enter(&mut self) -> Outcome {
        if self.current().is_none() && self.marks.is_empty() {
            return Outcome::Continue;
        }
        if self.pick {
            return Outcome::Accept(Choice::Remove, self.picked());
        }
        self.overlay = Overlay::Menu(Menu::new());
        Outcome::Continue
    }

    fn handle_menu(&mut self, mut menu: Menu, key: KeyEvent) -> Outcome {
        match menu.handle(key) {
            Pick::Open => self.overlay = Overlay::Menu(menu),
            Pick::Chosen(choice) => return Outcome::Accept(choice, self.picked()),
            Pick::Closed => {}
        }
        Outcome::Continue
    }

    fn handle_filter(&mut self, key: KeyEvent) -> Outcome {
        self.overlay = Overlay::Filter;
        if matches!(
            key.code,
            KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown
        ) {
            self.list.handle(key);
            return Outcome::Continue;
        }
        let before = self.filter.value().to_string();
        match self.filter.handle(key) {
            Edit::Editing if self.filter.value() != before => {
                self.list.select(0);
                self.refresh();
            }
            Edit::Editing => {}
            Edit::Submit => self.overlay = Overlay::None,
            Edit::Cancel => {
                self.overlay = Overlay::None;
                self.clear_filter();
            }
        }
        Outcome::Continue
    }

    fn navigate(&mut self, key: KeyEvent) {
        let Some(id) = self.current() else { return };
        let has_children = self.rows[id].has_children;
        match key.code {
            KeyCode::Right | KeyCode::Char('l') if has_children => {
                self.expanded.insert(id);
            }
            KeyCode::Char(' ') if has_children => self.toggle_open(id),
            KeyCode::Left | KeyCode::Char('h') => return self.collapse_or_ascend(id),
            _ => {
                self.list.handle(key);
                return;
            }
        }
        self.refresh();
    }

    fn collapse_or_ascend(&mut self, id: usize) {
        if self.expanded.remove(&id) {
            self.refresh();
        } else if let Some(parent) = self.rows[id].parent {
            self.select_row(parent);
        }
    }
}
