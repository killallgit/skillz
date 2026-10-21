//! The keys of the workspace's popups: help, the filter prompt, text questions, delete confirm.

use ratatui::crossterm::event::KeyEvent;

use super::ask::Ask;
use super::model::Act;
use super::state::{Home, Popup};
use crate::tui::help::{Help, Verdict as HelpVerdict};
use crate::tui::keymap::{self, Scope};
use crate::tui::widgets::confirm::{ConfirmDialog, Outcome as Verdict};
use crate::tui::widgets::input::{InputField, Outcome as Edit};

impl Home {
    pub(super) fn handle_help(&mut self, mut help: Help, key: KeyEvent) -> Option<Act> {
        if help.handle(key) == HelpVerdict::Open {
            self.popup = Popup::Help(help);
        }
        None
    }

    pub(super) fn handle_filter(&mut self, key: KeyEvent) -> Option<Act> {
        self.popup = Popup::Filter;
        if let Some(movement) = keymap::resolve(Scope::Filter, key) {
            self.pane_mut().list.apply(movement);
            return None;
        }
        let texts = self.texts();
        if self.pane_mut().edit(key, &texts) != Edit::Editing {
            self.popup = Popup::None;
        }
        None
    }

    pub(super) fn handle_input(
        &mut self,
        ask: Ask,
        mut field: InputField,
        key: KeyEvent,
    ) -> Option<Act> {
        let answer = field.value().trim().to_string();
        match field.handle(key) {
            Edit::Cancel => None,
            Edit::Submit if !answer.is_empty() => {
                ask.act(answer, self.project_id(), self.profile_id())
            }
            Edit::Submit | Edit::Editing => self.open(Popup::Input(ask, field)),
        }
    }

    pub(super) fn handle_delete(
        &mut self,
        profile: String,
        mut dialog: ConfirmDialog,
        key: KeyEvent,
    ) -> Option<Act> {
        match dialog.handle(key) {
            Verdict::Yes => Some(Act::DeleteProfile(profile)),
            Verdict::No => None,
            Verdict::Pending => self.open(Popup::Delete(profile, dialog)),
        }
    }
}
