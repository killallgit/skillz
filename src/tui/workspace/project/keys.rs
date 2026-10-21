//! The Project tab's keys.

use ratatui::crossterm::event::KeyEvent;

use super::picker::{Pick, Picker};
use super::rows::{Row, State};
use super::{Popup, ProjectTab};
use crate::tui::keymap::{self, Action, Scope};
use crate::tui::widgets::input::Outcome as Edit;
use crate::tui::workspace::act::Act;

impl ProjectTab {
    pub(super) fn handle_key(&mut self, key: KeyEvent) -> Option<Act> {
        match std::mem::take(&mut self.popup) {
            Popup::None => self.handle_idle(key),
            Popup::Filter => self.handle_filter(key),
            Popup::Profiles(picker) => self.handle_picker(picker, key),
        }
    }

    fn handle_idle(&mut self, key: KeyEvent) -> Option<Act> {
        match keymap::resolve(Scope::Project, key)? {
            Action::Filter => self.popup = Popup::Filter,
            Action::Back if self.outline.is_filtered() => self.outline.clear_filter(),
            Action::PickProfile => self.open_picker(),
            Action::ApplyProfile => {
                return self.compare.clone().map(|profile| Act::Apply { profile });
            }
            Action::Toggle | Action::Select => return self.activate(),
            movement => self.outline.navigate(movement),
        }
        None
    }

    fn open_picker(&mut self) {
        let names = self.profiles.iter().map(|p| p.name.clone()).collect();
        self.popup = Popup::Profiles(Picker::new(names, self.compare.as_deref()));
    }

    /// A section opens or closes; an item is installed or removed.
    fn activate(&mut self) -> Option<Act> {
        let id = self.outline.current()?;
        match &self.rows[id] {
            Row::Section { .. } => {
                self.outline.toggle_open(id);
                None
            }
            Row::Item(item) => Some(match &item.state {
                State::Installed(id) => Act::Remove { id: id.clone() },
                State::Available(entry) => Act::Install(entry.clone()),
            }),
        }
    }

    fn handle_filter(&mut self, key: KeyEvent) -> Option<Act> {
        self.popup = Popup::Filter;
        if let Some(movement) = keymap::resolve(Scope::Filter, key) {
            self.outline.step(movement);
            return None;
        }
        if self.outline.edit_filter(key) != Edit::Editing {
            self.popup = Popup::None;
        }
        None
    }

    fn handle_picker(&mut self, mut picker: Picker, key: KeyEvent) -> Option<Act> {
        match picker.handle(key) {
            Pick::Open => self.popup = Popup::Profiles(picker),
            Pick::Chosen(profile) => self.compare = profile,
            Pick::Closed => {}
        }
        None
    }
}
