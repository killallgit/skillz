//! The workspace around its tabs: the tab bar, the key bar, help and toasts, and the keys that
//! belong to no tab.

use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line;

use std::sync::mpsc::Receiver;

use super::act::Act;
use super::checks::Checks;
use super::library::LibraryTab;
use super::model::Model;
use super::profiles::ProfilesTab;
use super::project::ProjectTab;
use super::tab::Tab;
use crate::review::ItemEvent;
use crate::tui::filter;
use crate::tui::help::{Help, Verdict};
use crate::tui::keymap::{self, Action, Scope};
use crate::tui::theme::Theme;
use crate::tui::widgets::empty;
use crate::tui::widgets::keybar::KeyBar;
use crate::tui::widgets::tabs::Tabs;
use crate::tui::widgets::toast::Toasts;

#[cfg(test)]
mod tests;

const MIN_WIDTH: u16 = 40;
const MIN_HEIGHT: u16 = 10;

pub struct Workspace {
    tabs: Vec<Box<dyn Tab>>,
    bar: Tabs,
    toasts: Toasts,
    help: Option<Help>,
    checks: Checks,
}

impl Workspace {
    pub fn new(tabs: Vec<Box<dyn Tab>>) -> Self {
        let titles: Vec<&str> = tabs.iter().map(|t| t.title()).collect();
        Workspace {
            bar: Tabs::new(&titles),
            tabs,
            toasts: Toasts::default(),
            help: None,
            checks: Checks::default(),
        }
    }

    /// Mark the items whose ids arrive on `found` as having an update.
    pub fn watch_updates(&mut self, found: Receiver<String>) {
        self.checks.watch(found);
    }

    /// Drop the marks of items that were just updated.
    pub fn forget_updates(&mut self, ids: &[String]) {
        self.checks.forget(ids);
    }

    /// Show where an item's change stands on every tab.
    pub fn note(&mut self, event: &ItemEvent) {
        self.tabs.iter_mut().for_each(|tab| tab.note(event));
    }

    /// Move the animations on a frame.
    pub fn tick(&mut self) {
        self.tabs.iter_mut().for_each(|tab| tab.tick());
    }

    /// Project, Library and Profiles.
    pub fn standard() -> Self {
        Workspace::new(vec![
            Box::new(ProjectTab::new()),
            Box::new(LibraryTab::new()),
            Box::new(ProfilesTab::new()),
        ])
    }

    /// Give every tab the new snapshot and toast what the last act reported.
    pub fn load(&mut self, mut model: Model) {
        if let Some(notice) = model.notice.take() {
            self.toasts.push(notice);
        }
        self.tabs.iter_mut().for_each(|tab| tab.load(&model));
    }

    fn tab(&self) -> &dyn Tab {
        self.tabs[self.bar.selected()].as_ref()
    }

    fn tab_mut(&mut self) -> &mut dyn Tab {
        self.tabs[self.bar.selected()].as_mut()
    }

    fn scopes(&self) -> Vec<Scope> {
        let mut scopes = self.tab().scopes().to_vec();
        scopes.push(Scope::Workspace);
        scopes
    }

    pub fn handle(&mut self, key: KeyEvent) -> Option<Act> {
        self.toasts.dismiss();
        if keymap::resolve(Scope::Common, key) == Some(Action::Interrupt) {
            return Some(Act::Quit);
        }
        if let Some(help) = self.help.take() {
            return self.handle_help(help, key);
        }
        if self.tab().captures_keys() {
            return self.tab_mut().handle(key);
        }
        match keymap::resolve(Scope::Workspace, key) {
            Some(Action::Quit) => Some(Act::Quit),
            Some(Action::Help) => {
                self.help = Some(Help::new(&self.scopes()));
                None
            }
            Some(Action::NextTab) => self.switch(|bar| bar.next()),
            Some(Action::PrevTab) => self.switch(|bar| bar.previous()),
            Some(Action::GoTab(at)) => self.switch(|bar| bar.select(at.into())),
            _ => self.tab_mut().handle(key),
        }
    }

    fn switch(&mut self, change: impl FnOnce(&mut Tabs)) -> Option<Act> {
        change(&mut self.bar);
        None
    }

    fn handle_help(&mut self, mut help: Help, key: KeyEvent) -> Option<Act> {
        if help.handle(key) == Verdict::Open {
            self.help = Some(help);
        }
        None
    }

    pub fn draw(&mut self, frame: &mut Frame, theme: &Theme) {
        let area = frame.area();
        if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
            return draw_too_small(frame, area, theme);
        }
        let [bar, body, foot] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .areas(area);
        let outdated = self.checks.outdated().clone();
        self.tabs
            .iter_mut()
            .for_each(|tab| tab.mark_outdated(&outdated));
        self.bar.render(frame, bar, theme);
        self.tabs[self.bar.selected()].draw(frame, body, theme);
        self.draw_footer(frame, foot, theme);
        self.toasts.render(frame, area, theme);
        if let Some(help) = &mut self.help {
            help.render(frame, theme);
        }
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        match self.tab().prompt() {
            Some(field) => filter::draw_prompt(frame, area, field, theme),
            None => KeyBar::new(&keymap::hints(&self.scopes())).render(frame, area, theme),
        }
    }
}

fn draw_too_small(frame: &mut Frame, area: Rect, theme: &Theme) {
    let lines = vec![
        Line::styled("Terminal too small", theme.warn),
        Line::styled(format!("needs {MIN_WIDTH}x{MIN_HEIGHT}"), theme.muted),
    ];
    empty::render(frame, area, lines);
}
