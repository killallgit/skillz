//! The Profiles tab's keys.

use ratatui::crossterm::event::KeyEvent;

use super::chooser::{Choice, Chooser};
use super::naming::{Naming, Purpose};
use super::{Focus, Popup, ProfilesTab};
use crate::tui::keymap::{self, Action, Scope};
use crate::tui::widgets::confirm::{ConfirmDialog, Outcome as Answer};
use crate::tui::widgets::input::Outcome as Edit;
use crate::tui::workspace::act::Act;

impl ProfilesTab {
    pub(super) fn handle_key(&mut self, key: KeyEvent) -> Option<Act> {
        match std::mem::take(&mut self.popup) {
            Popup::None => self.handle_idle(key),
            Popup::Filter => self.handle_filter(key),
            Popup::Name(naming) => self.handle_name(naming, key),
            Popup::Build(chooser) => self.handle_chooser(chooser, key),
            Popup::Delete(name, dialog) => self.handle_delete(name, dialog, key),
        }
    }

    fn handle_idle(&mut self, key: KeyEvent) -> Option<Act> {
        match keymap::resolve(Scope::Profiles, key)? {
            Action::Filter => self.popup = Popup::Filter,
            Action::Back if self.list.is_filtered() => self.clear_filter(),
            Action::Back | Action::Left => self.focus = Focus::List,
            Action::Right => self.focus = Focus::Entries,
            Action::ApplyProfile => {
                return self.current_name().map(|profile| Act::Apply { profile });
            }
            Action::ApplyExact => {
                return self
                    .current_name()
                    .map(|profile| Act::ApplyExact { profile });
            }
            Action::NewProfile => self.popup = Popup::Name(Naming::new(Purpose::New)),
            Action::SaveCurrent => self.popup = Popup::Name(Naming::new(Purpose::SaveCurrent)),
            Action::Build => self.open_chooser(),
            Action::Delete => self.ask_delete(),
            movement => self.move_cursor(movement),
        }
        None
    }

    fn move_cursor(&mut self, movement: Action) {
        match self.focus {
            Focus::List => {
                self.list.navigate(movement);
                self.show_entries();
            }
            Focus::Entries => self.entries.navigate(movement),
        }
    }

    fn clear_filter(&mut self) {
        self.list.clear_filter();
        self.show_entries();
    }

    fn open_chooser(&mut self) {
        if let Some(profile) = self.current() {
            self.popup = Popup::Build(Box::new(Chooser::new(profile, &self.library)));
        }
    }

    fn ask_delete(&mut self) {
        if let Some(name) = self.current_name() {
            let prompt = format!("Delete profile {name}?");
            self.popup = Popup::Delete(name, ConfirmDialog::new("Delete", prompt));
        }
    }

    fn handle_filter(&mut self, key: KeyEvent) -> Option<Act> {
        self.popup = Popup::Filter;
        if let Some(movement) = keymap::resolve(Scope::Filter, key) {
            self.list.step(movement);
        } else if self.list.edit_filter(key) != Edit::Editing {
            self.popup = Popup::None;
        }
        self.show_entries();
        None
    }

    fn handle_name(&mut self, mut naming: Naming, key: KeyEvent) -> Option<Act> {
        match naming.field.handle(key) {
            Edit::Editing => {
                self.popup = Popup::Name(naming);
                None
            }
            Edit::Submit => naming.act(),
            Edit::Cancel => None,
        }
    }

    fn handle_chooser(&mut self, mut chooser: Box<Chooser>, key: KeyEvent) -> Option<Act> {
        match chooser.handle(key) {
            Choice::Open => self.popup = Popup::Build(chooser),
            Choice::Done(entries) => {
                let profile = chooser.profile().to_string();
                return Some(Act::SetEntries { profile, entries });
            }
            Choice::Closed => {}
        }
        None
    }

    fn handle_delete(
        &mut self,
        name: String,
        mut dialog: ConfirmDialog,
        key: KeyEvent,
    ) -> Option<Act> {
        match dialog.handle(key) {
            Answer::Pending => self.popup = Popup::Delete(name, dialog),
            Answer::Yes => return Some(Act::DeleteProfile { name }),
            Answer::No => {}
        }
        None
    }
}
