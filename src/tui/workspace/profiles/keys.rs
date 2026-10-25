//! The Profiles tab's keys.

use ratatui::crossterm::event::KeyEvent;

use super::editor::{Choice, Editor};
use super::{Popup, ProfilesTab};
use crate::tui::keymap::{self, Action, Scope};
use crate::tui::widgets::confirm::{ConfirmDialog, Outcome as Answer};
use crate::tui::widgets::input::Outcome as Edit;
use crate::workspace::Act;

impl ProfilesTab {
    pub(super) fn handle_key(&mut self, key: KeyEvent) -> Option<Act> {
        match std::mem::take(&mut self.popup) {
            Popup::None => self.handle_idle(key),
            Popup::Filter => self.handle_filter(key),
            Popup::Edit(editor) => self.handle_editor(editor, key),
            Popup::Delete(name, dialog) => self.handle_delete(name, dialog, key),
        }
    }

    fn handle_idle(&mut self, key: KeyEvent) -> Option<Act> {
        match keymap::resolve(Scope::Profiles, key)? {
            Action::Filter => self.popup = Popup::Filter,
            Action::Back if self.list.is_filtered() => self.clear_filter(),
            Action::NewProfile => {
                self.popup = Popup::Edit(Box::new(Editor::creating(&self.library)));
            }
            Action::Build => self.open_editor(),
            Action::Delete => self.ask_delete(),
            movement => self.move_cursor(movement),
        }
        None
    }

    fn move_cursor(&mut self, movement: Action) {
        self.list.navigate(movement);
    }

    fn clear_filter(&mut self) {
        self.list.clear_filter();
    }

    fn open_editor(&mut self) {
        if let Some(profile) = self.current() {
            self.popup = Popup::Edit(Box::new(Editor::editing(profile, &self.library)));
        }
    }

    fn ask_delete(&mut self) {
        if let Some(name) = self.current_name() {
            let prompt = format!("Delete profile {name}?");
            self.popup = Popup::Delete(name, ConfirmDialog::new("Delete", prompt));
        }
    }

    fn handle_filter(&mut self, key: KeyEvent) -> Option<Act> {
        self.popup = if self.list.filter_key(key) == Edit::Editing {
            Popup::Filter
        } else {
            Popup::None
        };
        None
    }

    fn handle_editor(&mut self, mut editor: Box<Editor>, key: KeyEvent) -> Option<Act> {
        match editor.handle(key) {
            Choice::Open => self.popup = Popup::Edit(editor),
            Choice::Save {
                name, new: true, ..
            } if self.profiles.iter().any(|row| row.profile.name == name) => {
                editor.refuse("a profile with that name exists");
                self.popup = Popup::Edit(editor);
            }
            Choice::Save {
                name,
                entries,
                new: true,
            } => return Some(Act::CreateProfile { name, entries }),
            Choice::Save { name, entries, .. } => {
                return Some(Act::SetEntries {
                    profile: name,
                    entries,
                });
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
