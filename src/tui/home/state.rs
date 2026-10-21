//! The workspace's state and its keys: a pure machine that only says what the user asked for.

use std::path::PathBuf;

use ratatui::crossterm::event::KeyEvent;

use super::ask::Ask;
use super::detail::Detail;
use super::layout::Mode;
use super::model::{Act, Focus, Model, Pos, ProjectRow};
use super::mouse::Click;
use super::pane::Pane;
use crate::tui::help::Help;
use crate::tui::keymap::{self, Action, Scope};
use crate::tui::widgets::Outcome as Moved;
use crate::tui::widgets::confirm::ConfirmDialog;
use crate::tui::widgets::input::InputField;
use crate::tui::widgets::tabs::Tabs;
use crate::tui::widgets::toast::Toasts;

/// The tab that shows the detail pane when only one pane fits.
const DETAIL_TAB: usize = 2;

/// Which popup, if any, has the keys.
#[derive(Debug, Clone, Default)]
pub(super) enum Popup {
    #[default]
    None,
    Input(Ask, InputField),
    /// Deleting the named profile waits for a yes.
    Delete(String, ConfirmDialog),
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
    pub(super) last_click: Option<Click>,
    pub(super) detail: Detail,
}

impl Home {
    pub fn new(mut model: Model, pos: Pos) -> Self {
        let mut projects = Pane::new(model.projects.len());
        projects.select_row(pos.project);
        let mut profiles = Pane::new(model.profiles.len());
        profiles.select_row(pos.profile);
        let mut toasts = Toasts::default();
        if let Some(notice) = model.notice.take() {
            toasts.push(notice);
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
            last_click: None,
            detail: Detail::default(),
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
            Action::Help => self.open(Popup::Help(Help::new(&[Scope::Home]))),
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
        let profile = self.profile_id();
        match action {
            Action::NewProfile => self.ask(Ask::NewProfile),
            Action::SaveProfile if self.usable_project().is_some() => self.ask(Ask::SaveProfile),
            Action::AddTyped if profile.is_some() => self.ask(Ask::AddEntry),
            Action::AddInstalled => self.add_installed(),
            Action::RemoveEntries => profile.map(|profile| Act::RemoveEntries { profile }),
            Action::Delete => self.delete(),
            Action::DetailUp | Action::DetailDown => self.scroll_detail(action),
            movement => self.move_cursor(movement),
        }
    }

    fn scroll_detail(&mut self, action: Action) -> Option<Act> {
        self.detail.page(action == Action::DetailDown);
        None
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

    pub(super) fn focus_on(&mut self, focus: Focus) {
        self.focus = focus;
        self.tabs.select(tab_of(focus));
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

    pub(super) fn project_id(&self) -> Option<PathBuf> {
        let at = self.projects.row()?;
        Some(self.model.projects[at].path.clone())
    }

    pub(super) fn profile_id(&self) -> Option<String> {
        let at = self.profiles.row()?;
        Some(self.model.profiles[at].name.clone())
    }

    fn usable_project(&self) -> Option<PathBuf> {
        let row = &self.model.projects[self.projects.row()?];
        (!row.missing).then(|| row.path.clone())
    }

    fn usable_profile(&self) -> Option<String> {
        let row = &self.model.profiles[self.profiles.row()?];
        (!row.broken).then(|| row.name.clone())
    }

    pub(super) fn apply(&self) -> Option<Act> {
        let project = self.usable_project()?;
        let profile = self.usable_profile()?;
        Some(Act::Apply { project, profile })
    }

    fn add_installed(&self) -> Option<Act> {
        let profile = self.profile_id()?;
        let project = self.usable_project()?;
        Some(Act::AddInstalled { profile, project })
    }

    pub(super) fn projects_using(&self, profile: &str) -> usize {
        let loaded = |p: &&ProjectRow| p.profile.as_deref() == Some(profile);
        self.model.projects.iter().filter(loaded).count()
    }

    fn delete(&mut self) -> Option<Act> {
        match self.focus {
            Focus::Projects => None,
            Focus::Profiles => {
                let profile = self.profile_id()?;
                let users = self.projects_using(&profile);
                let prompt = match users {
                    0 => format!("Delete profile {profile}?"),
                    n => format!("Delete profile {profile}? {n} project(s) have it loaded."),
                };
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
