//! Keys for the tree: `TreeState::handle` turns a key into a state change or an `Outcome`.

use ratatui::crossterm::event::KeyEvent;

use super::menu::{Menu, Pick};
use super::model::{Overlay, TreeState};
use crate::prompt::{Choice, Picked};
use crate::tui::help::{Help, Verdict};
use crate::tui::keymap::{self, Action, Scope};
use crate::tui::widgets::input::Outcome as Edit;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Continue,
    Quit,
    Accept(Choice, Picked),
}

impl TreeState {
    pub fn handle(&mut self, key: KeyEvent) -> Outcome {
        if keymap::resolve(Scope::Common, key) == Some(Action::Interrupt) {
            return Outcome::Quit;
        }
        match std::mem::take(&mut self.overlay) {
            Overlay::None => self.handle_browsing(key),
            Overlay::Filter => self.handle_filter(key),
            Overlay::Menu(menu) => self.handle_menu(menu, key),
            Overlay::Info(id) => self.handle_info(id, key),
            Overlay::Help(help) => self.handle_help(help, key),
        }
    }

    fn handle_help(&mut self, mut help: Help, key: KeyEvent) -> Outcome {
        if help.handle(key) == Verdict::Open {
            self.overlay = Overlay::Help(help);
        }
        Outcome::Continue
    }

    fn handle_info(&mut self, id: usize, key: KeyEvent) -> Outcome {
        let closes = matches!(
            keymap::resolve(Scope::Tree, key),
            Some(Action::Back | Action::Info | Action::Select)
        );
        if !closes {
            self.overlay = Overlay::Info(id);
        }
        Outcome::Continue
    }

    fn handle_browsing(&mut self, key: KeyEvent) -> Outcome {
        let Some(action) = keymap::resolve(Scope::Tree, key) else {
            return Outcome::Continue;
        };
        match action {
            Action::Filter => self.overlay = Overlay::Filter,
            Action::Help => self.overlay = Overlay::Help(Help::new(Scope::Tree)),
            Action::Mark => self.toggle_mark(),
            Action::Info => self.open_info(),
            Action::Select => return self.enter(),
            Action::Back if self.filter_is_active() => self.clear_filter(),
            Action::Back | Action::Quit => return Outcome::Quit,
            action => self.navigate(action),
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
        if let Some(movement) = keymap::resolve(Scope::Filter, key) {
            self.list.apply(movement);
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

    fn navigate(&mut self, action: Action) {
        let Some(id) = self.current() else { return };
        let has_children = self.rows[id].has_children;
        match action {
            Action::Right if has_children => {
                self.expanded.insert(id);
            }
            Action::Toggle if has_children => self.toggle_open(id),
            Action::Left => return self.collapse_or_ascend(id),
            movement => {
                self.list.apply(movement);
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
