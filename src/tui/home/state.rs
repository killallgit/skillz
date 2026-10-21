//! The workspace's state and its keys: a pure machine that only says what the user asked for.

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::ask::Ask;
use super::model::{Act, Focus, Model, Pos};
use crate::tui::keys::is_ctrl_c;
use crate::tui::widgets::confirm::{ConfirmDialog, Outcome as Verdict};
use crate::tui::widgets::input::{InputField, Outcome as Edit};
use crate::tui::widgets::list::ListView;
use crate::tui::widgets::toast::{Level, Toasts};

/// Which popup, if any, has the keys.
#[derive(Debug, Clone, Default)]
pub(super) enum Popup {
    #[default]
    None,
    Input(Ask, InputField),
    /// Deleting the profile at this index waits for a yes.
    Delete(usize, ConfirmDialog),
    Help,
}

pub struct Home {
    pub(super) model: Model,
    pub(super) focus: Focus,
    pub(super) projects: ListView,
    pub(super) profiles: ListView,
    pub(super) popup: Popup,
    pub(super) toasts: Toasts,
}

impl Home {
    pub fn new(mut model: Model, pos: Pos) -> Self {
        let mut projects = ListView::new(model.projects.len());
        projects.select(pos.project);
        let mut profiles = ListView::new(model.profiles.len());
        profiles.select(pos.profile);
        let mut toasts = Toasts::default();
        if let Some(notice) = model.notice.take() {
            toasts.push(Level::Info, notice);
        }
        Home {
            model,
            focus: pos.focus,
            projects,
            profiles,
            popup: Popup::None,
            toasts,
        }
    }

    pub fn pos(&self) -> Pos {
        Pos {
            focus: self.focus,
            project: self.projects.selected().unwrap_or(0),
            profile: self.profiles.selected().unwrap_or(0),
        }
    }

    pub fn handle(&mut self, key: KeyEvent) -> Option<Act> {
        if is_ctrl_c(key) {
            return Some(Act::Quit);
        }
        self.toasts.dismiss();
        match std::mem::take(&mut self.popup) {
            Popup::None => self.handle_idle(key),
            Popup::Input(ask, field) => self.handle_input(ask, field, key),
            Popup::Delete(profile, dialog) => self.handle_delete(profile, dialog, key),
            Popup::Help => None,
        }
    }

    fn handle_idle(&mut self, key: KeyEvent) -> Option<Act> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Some(Act::Quit),
            KeyCode::Tab | KeyCode::Left | KeyCode::Right | KeyCode::Char('h' | 'l') => {
                self.focus = self.focus.other();
                None
            }
            KeyCode::Char('?') => self.open(Popup::Help),
            KeyCode::Enter => self.apply(),
            KeyCode::Char(c @ ('u' | 'a' | 'n' | 's' | 'i' | 'r' | 't' | 'd')) => self.command(c),
            _ => {
                self.focused_list().handle(key);
                None
            }
        }
    }

    fn command(&mut self, command: char) -> Option<Act> {
        let profile = self.profiles.selected();
        match command {
            'u' => self.loaded_project().map(|project| Act::Unload { project }),
            'a' => self.ask(Ask::AddProject),
            'n' => self.ask(Ask::NewProfile),
            's' if self.usable_project().is_some() => self.ask(Ask::SaveProfile),
            't' if profile.is_some() => self.ask(Ask::AddEntry),
            'i' => self.add_installed(),
            'r' => profile.map(|profile| Act::RemoveEntries { profile }),
            'd' => self.delete(),
            _ => None,
        }
    }

    fn focused_list(&mut self) -> &mut ListView {
        match self.focus {
            Focus::Projects => &mut self.projects,
            Focus::Profiles => &mut self.profiles,
        }
    }

    fn open(&mut self, popup: Popup) -> Option<Act> {
        self.popup = popup;
        None
    }

    fn ask(&mut self, ask: Ask) -> Option<Act> {
        self.open(Popup::Input(ask, InputField::new(ask.placeholder())))
    }

    fn usable_project(&self) -> Option<usize> {
        let at = self.projects.selected()?;
        (!self.model.projects[at].missing).then_some(at)
    }

    fn loaded_project(&self) -> Option<usize> {
        let at = self.projects.selected()?;
        self.model.projects[at].loaded.then_some(at)
    }

    fn apply(&self) -> Option<Act> {
        let project = self.usable_project()?;
        let profile = self.profiles.selected()?;
        (!self.model.profiles[profile].broken).then_some(Act::Apply { project, profile })
    }

    fn add_installed(&self) -> Option<Act> {
        let profile = self.profiles.selected()?;
        let project = self.usable_project()?;
        Some(Act::AddInstalled { profile, project })
    }

    fn delete(&mut self) -> Option<Act> {
        match self.focus {
            Focus::Projects => self.projects.selected().map(Act::ForgetProject),
            Focus::Profiles => {
                let profile = self.profiles.selected()?;
                let prompt = format!("Delete profile {}?", self.model.profiles[profile].name);
                let dialog = ConfirmDialog::new("Delete profile", prompt).destructive();
                self.open(Popup::Delete(profile, dialog))
            }
        }
    }

    fn handle_input(&mut self, ask: Ask, mut field: InputField, key: KeyEvent) -> Option<Act> {
        let answer = field.value().trim().to_string();
        match field.handle(key) {
            Edit::Cancel => None,
            Edit::Submit if !answer.is_empty() => {
                let pos = self.pos();
                Some(ask.act(answer, pos.project, pos.profile))
            }
            Edit::Submit | Edit::Editing => self.open(Popup::Input(ask, field)),
        }
    }

    fn handle_delete(
        &mut self,
        profile: usize,
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
