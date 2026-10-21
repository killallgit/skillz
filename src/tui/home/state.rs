//! The workspace's state and its keys: a pure machine that only says what the user asked for.

use ratatui::crossterm::event::KeyEvent;

use super::ask::Ask;
use super::layout::Mode;
use super::model::{Act, Focus, Model, Pos};
use super::pane::Pane;
use crate::tui::help::Help;
use crate::tui::keymap::{self, Action, Scope};
use crate::tui::widgets::Outcome as Moved;
use crate::tui::widgets::confirm::ConfirmDialog;
use crate::tui::widgets::input::InputField;
use crate::tui::widgets::tabs::Tabs;
use crate::tui::widgets::toast::{Level, Toasts};

/// The tab that shows the detail pane when only one pane fits.
const DETAIL_TAB: usize = 2;

/// Which popup, if any, has the keys.
#[derive(Debug, Clone, Default)]
pub(super) enum Popup {
    #[default]
    None,
    Input(Ask, InputField),
    /// Deleting the profile at this index waits for a yes.
    Delete(usize, ConfirmDialog),
    Help(Help),
    /// Typing into the focused pane's filter.
    Filter,
}

pub struct Home {
    pub(super) model: Model,
    pub(super) focus: Focus,
    pub(super) projects: Pane,
    pub(super) profiles: Pane,
    pub(super) popup: Popup,
    pub(super) toasts: Toasts,
    pub(super) mode: Mode,
    pub(super) tabs: Tabs,
}

impl Home {
    pub fn new(mut model: Model, pos: Pos) -> Self {
        let mut projects = Pane::new(model.projects.len());
        projects.select_row(pos.project);
        let mut profiles = Pane::new(model.profiles.len());
        profiles.select_row(pos.profile);
        let mut toasts = Toasts::default();
        if let Some(notice) = model.notice.take() {
            toasts.push(Level::Info, notice);
        }
        let mut tabs = Tabs::new(&["Projects", "Profiles", "Detail"]);
        tabs.select(tab_of(pos.focus));
        Home {
            model,
            focus: pos.focus,
            projects,
            profiles,
            popup: Popup::None,
            toasts,
            mode: Mode::default(),
            tabs,
        }
    }

    pub fn pos(&self) -> Pos {
        Pos {
            focus: self.focus,
            project: self.projects.row().unwrap_or(0),
            profile: self.profiles.row().unwrap_or(0),
        }
    }

    pub fn handle(&mut self, key: KeyEvent) -> Option<Act> {
        if keymap::resolve(Scope::Common, key) == Some(Action::Interrupt) {
            return Some(Act::Quit);
        }
        self.toasts.dismiss();
        match std::mem::take(&mut self.popup) {
            Popup::None => self.handle_idle(key),
            Popup::Input(ask, field) => self.handle_input(ask, field, key),
            Popup::Delete(profile, dialog) => self.handle_delete(profile, dialog, key),
            Popup::Help(help) => self.handle_help(help, key),
            Popup::Filter => self.handle_filter(key),
        }
    }

    fn handle_idle(&mut self, key: KeyEvent) -> Option<Act> {
        if self.mode == Mode::Single && self.tabs.handle(key) == Moved::Changed {
            self.focus_follows_tab();
            return None;
        }
        match keymap::resolve(Scope::Home, key)? {
            Action::Quit => Some(Act::Quit),
            Action::Back => self.back(),
            Action::SwitchPane => self.switch_pane(),
            Action::Help => self.open(Popup::Help(Help::new(Scope::Home))),
            Action::Filter => self.open(Popup::Filter),
            Action::Select => self.apply(),
            action => self.command(action),
        }
    }

    /// Esc: clear the focused pane's filter first; with none, leave the screen.
    fn back(&mut self) -> Option<Act> {
        if !self.pane().is_filtered() {
            return Some(Act::Quit);
        }
        let texts = self.texts();
        self.pane_mut().clear_filter(&texts);
        None
    }

    fn command(&mut self, action: Action) -> Option<Act> {
        let profile = self.profiles.row();
        match action {
            Action::Unload => self.loaded_project().map(|project| Act::Unload { project }),
            Action::AddProject => self.ask(Ask::AddProject),
            Action::NewProfile => self.ask(Ask::NewProfile),
            Action::SaveProfile if self.usable_project().is_some() => self.ask(Ask::SaveProfile),
            Action::AddTyped if profile.is_some() => self.ask(Ask::AddEntry),
            Action::AddInstalled => self.add_installed(),
            Action::RemoveEntries => profile.map(|profile| Act::RemoveEntries { profile }),
            Action::Delete => self.delete(),
            movement => self.move_cursor(movement),
        }
    }

    fn move_cursor(&mut self, movement: Action) -> Option<Act> {
        let hidden = self.mode == Mode::Single && self.tabs.selected() == DETAIL_TAB;
        if !hidden {
            self.pane_mut().list.apply(movement);
        }
        None
    }

    fn switch_pane(&mut self) -> Option<Act> {
        self.focus = self.focus.other();
        self.tabs.select(tab_of(self.focus));
        None
    }

    fn focus_follows_tab(&mut self) {
        match self.tabs.selected() {
            0 => self.focus = Focus::Projects,
            1 => self.focus = Focus::Profiles,
            _ => {}
        }
    }

    pub(super) fn pane(&self) -> &Pane {
        match self.focus {
            Focus::Projects => &self.projects,
            Focus::Profiles => &self.profiles,
        }
    }

    pub(super) fn pane_mut(&mut self) -> &mut Pane {
        match self.focus {
            Focus::Projects => &mut self.projects,
            Focus::Profiles => &mut self.profiles,
        }
    }

    /// What the focused pane's filter matches against, one entry per model row.
    pub(super) fn texts(&self) -> Vec<String> {
        match self.focus {
            Focus::Projects => self
                .model
                .projects
                .iter()
                .map(|p| p.search_text())
                .collect(),
            Focus::Profiles => self.model.profiles.iter().map(|p| p.name.clone()).collect(),
        }
    }

    pub(super) fn open(&mut self, popup: Popup) -> Option<Act> {
        self.popup = popup;
        None
    }

    pub(super) fn ask(&mut self, ask: Ask) -> Option<Act> {
        self.open(Popup::Input(ask, InputField::new(ask.placeholder())))
    }

    fn usable_project(&self) -> Option<usize> {
        let at = self.projects.row()?;
        (!self.model.projects[at].missing).then_some(at)
    }

    fn loaded_project(&self) -> Option<usize> {
        let at = self.projects.row()?;
        self.model.projects[at].loaded().then_some(at)
    }

    fn apply(&self) -> Option<Act> {
        let project = self.usable_project()?;
        let profile = self.profiles.row()?;
        (!self.model.profiles[profile].broken).then_some(Act::Apply { project, profile })
    }

    fn add_installed(&self) -> Option<Act> {
        let profile = self.profiles.row()?;
        let project = self.usable_project()?;
        Some(Act::AddInstalled { profile, project })
    }

    fn delete(&mut self) -> Option<Act> {
        match self.focus {
            Focus::Projects => self.projects.row().map(Act::ForgetProject),
            Focus::Profiles => {
                let profile = self.profiles.row()?;
                let prompt = format!("Delete profile {}?", self.model.profiles[profile].name);
                let dialog = ConfirmDialog::new("Delete profile", prompt).destructive();
                self.open(Popup::Delete(profile, dialog))
            }
        }
    }
}

fn tab_of(focus: Focus) -> usize {
    match focus {
        Focus::Projects => 0,
        Focus::Profiles => 1,
    }
}
