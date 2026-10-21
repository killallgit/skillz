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
            Action::Back if !self.pending.is_empty() => self.pending.clear(),
            Action::Back if self.outline.is_filtered() => self.outline.clear_filter(),
            Action::PickProfile => self.open_picker(),
            Action::ApplyProfile => {
                return self.compare.clone().map(|profile| Act::Apply { profile });
            }
            Action::Toggle => self.toggle(),
            Action::Select => return self.select(),
            movement => self.outline.navigate(movement),
        }
        None
    }

    fn open_picker(&mut self) {
        let names = self.profiles.iter().map(|p| p.name.clone()).collect();
        self.popup = Popup::Profiles(Picker::new(names, self.compare.as_deref()));
    }

    fn toggle(&mut self) {
        match self.current_state() {
            Some(state) => self.pending.toggle(&state),
            None => self.fold(),
        }
    }

    /// Apply what is picked; with nothing picked, a section opens or closes.
    fn select(&mut self) -> Option<Act> {
        if self.pending.is_empty() {
            self.fold();
            return None;
        }
        Some(self.pending.act())
    }

    fn current_state(&self) -> Option<State> {
        match &self.rows[self.outline.current()?] {
            Row::Item(item) => Some(item.state.clone()),
            Row::Section { .. } => None,
        }
    }

    fn fold(&mut self) {
        let Some(id) = self.outline.current() else {
            return;
        };
        if matches!(self.rows[id], Row::Section { .. }) {
            self.outline.toggle_open(id);
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
