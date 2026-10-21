//! The keys of the Project and Global tabs.

use ratatui::crossterm::event::KeyEvent;

use super::picker::{Pick, Picker};
use super::rows::{Item, Row};
use super::{InstalledTab, Popup};
use crate::tui::keymap::{self, Action};
use crate::tui::widgets::input::Outcome as Edit;
use crate::tui::workspace::Tab;
use crate::tui::workspace::act::Act;

impl InstalledTab {
    pub(super) fn handle_key(&mut self, key: KeyEvent) -> Option<Act> {
        match std::mem::take(&mut self.popup) {
            Popup::None => self.handle_idle(key),
            Popup::Filter => self.handle_filter(key),
            Popup::Profiles(picker) => self.handle_picker(picker, key),
            Popup::AlsoGlobal(dialog, proposal) => self.handle_also_global(dialog, proposal, key),
        }
    }

    fn handle_idle(&mut self, key: KeyEvent) -> Option<Act> {
        match keymap::resolve_in(self.scopes(), key)? {
            Action::Filter => self.popup = Popup::Filter,
            Action::Back if !self.pending.is_empty() => self.pending.clear(),
            Action::Back if self.outline.is_filtered() => self.outline.clear_filter(),
            Action::PickProfile => self.open_picker(),
            Action::ApplyProfile => {
                return self.compare.clone().map(|profile| Act::Apply { profile });
            }
            Action::Update => return self.update(),
            Action::Install => return self.install_current(),
            Action::Remove => return self.remove_current(),
            Action::Toggle => self.toggle(),
            Action::Select => return self.select(),
            movement => self.outline.navigate(movement),
        }
        None
    }

    /// Update the highlighted installed item, or the installed items of the highlighted section.
    /// Anything that cannot be updated is left alone without a word.
    fn update(&self) -> Option<Act> {
        let ids = self.updatable_ids();
        let scope = self.scope;
        (!ids.is_empty()).then_some(Act::Update { scope, ids })
    }

    fn open_picker(&mut self) {
        let names = self.profiles.iter().map(|p| p.name.clone()).collect();
        self.popup = Popup::Profiles(Picker::new(names, self.compare.as_deref()));
    }

    fn toggle(&mut self) {
        match self.current_item().map(|item| item.state.clone()) {
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
        let (install, remove) = self.pending.picks();
        self.propose(install, remove)
    }

    pub(super) fn current_item(&self) -> Option<&Item> {
        match &self.rows[self.outline.current()?] {
            Row::Item(item) => Some(item),
            _ => None,
        }
    }

    fn fold(&mut self) {
        let Some(id) = self.outline.current() else {
            return;
        };
        if !matches!(self.rows[id], Row::Item(_) | Row::Owned(_)) {
            self.outline.toggle_open(id);
        }
    }

    fn handle_filter(&mut self, key: KeyEvent) -> Option<Act> {
        self.popup = Popup::Filter;
        if let Some(movement) = keymap::resolve(keymap::Scope::Filter, key) {
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
