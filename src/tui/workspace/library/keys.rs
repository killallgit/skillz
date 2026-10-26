//! The Library tab's keys.

use ratatui::crossterm::event::KeyEvent;

use super::install::{InstallDialog, Verdict};
use super::rows::Row;
use super::{LibraryTab, Popup};
use crate::app::workspace::Act;
use crate::tui::keymap::{self, Action, Scope};
use crate::tui::widgets::confirm::{ConfirmDialog, Outcome as Answer};
use crate::tui::widgets::input::{InputField, Outcome as Edit};
use skillz_harness::Spec;

const ADD_PLACEHOLDER: &str =
    "skill SOURCE NAMES | plugin SOURCE MARKETPLACE NAME | hook EVENT COMMAND | mcp NAME COMMAND";

impl LibraryTab {
    pub(super) fn handle_key(&mut self, key: KeyEvent) -> Option<Act> {
        match std::mem::take(&mut self.popup) {
            Popup::None => self.handle_idle(key),
            Popup::Filter => self.handle_filter(key),
            Popup::Add(field) => self.handle_add(field, key),
            Popup::Remove(dialog, entry) => self.handle_remove(dialog, entry, key),
            Popup::Install(dialog, entry) => self.handle_install(dialog, entry, key),
        }
    }

    fn handle_idle(&mut self, key: KeyEvent) -> Option<Act> {
        match keymap::resolve(Scope::Library, key)? {
            Action::Filter => self.popup = Popup::Filter,
            Action::Back if self.outline.is_filtered() => self.outline.clear_filter(),
            Action::Add => self.popup = Popup::Add(InputField::new(ADD_PLACEHOLDER)),
            Action::Install => self.ask_to_install(),
            Action::Delete => self.ask_to_remove(),
            Action::Toggle | Action::Select => self.toggle_group(),
            movement => self.outline.navigate(movement),
        }
        None
    }

    fn current_entry(&self) -> Option<&Spec> {
        match &self.rows[self.outline.current()?] {
            Row::Item(item) => Some(&item.library.entry),
            Row::Agents | Row::Package { .. } => None,
        }
    }

    fn ask_to_remove(&mut self) {
        if let Some(entry) = self.current_entry().cloned() {
            let dialog = ConfirmDialog::new("Remove from library", format!("Remove {entry}?"));
            self.popup = Popup::Remove(dialog, entry);
        }
    }

    fn ask_to_install(&mut self) {
        let item = self.outline.current().and_then(|id| match &self.rows[id] {
            Row::Item(item) => Some(item),
            Row::Agents | Row::Package { .. } => None,
        });
        if let Some(item) = item {
            let dialog = InstallDialog::new(
                &item.library.short_label(),
                &item.library.entry,
                &self.agents,
            );
            self.popup = Popup::Install(dialog, item.library.entry.clone());
        }
    }

    fn handle_install(
        &mut self,
        mut dialog: InstallDialog,
        entry: Spec,
        key: KeyEvent,
    ) -> Option<Act> {
        match dialog.handle(key) {
            Verdict::Open => self.popup = Popup::Install(dialog, entry),
            Verdict::Cancel => {}
            Verdict::Install { scope, agents } => {
                let entry = entry.with_agents(agents);
                return Some(Act::LibraryInstall { scope, entry });
            }
        }
        None
    }

    fn toggle_group(&mut self) {
        let group = self
            .outline
            .current()
            .filter(|&id| matches!(self.rows[id], Row::Agents | Row::Package { .. }));
        if let Some(id) = group {
            self.outline.toggle_open(id);
        }
    }

    fn handle_filter(&mut self, key: KeyEvent) -> Option<Act> {
        self.popup = if self.outline.filter_key(key) == Edit::Editing {
            Popup::Filter
        } else {
            Popup::None
        };
        None
    }

    fn handle_add(&mut self, mut field: InputField, key: KeyEvent) -> Option<Act> {
        match field.handle(key) {
            Edit::Editing => self.popup = Popup::Add(field),
            Edit::Submit => {
                let spec = field.value().trim().to_string();
                return (!spec.is_empty()).then_some(Act::LibraryAdd { spec });
            }
            Edit::Cancel => {}
        }
        None
    }

    fn handle_remove(
        &mut self,
        mut dialog: ConfirmDialog,
        entry: Spec,
        key: KeyEvent,
    ) -> Option<Act> {
        match dialog.handle(key) {
            Answer::Pending => self.popup = Popup::Remove(dialog, entry),
            Answer::Yes => return Some(Act::LibraryRemove(entry)),
            Answer::No => {}
        }
        None
    }
}
