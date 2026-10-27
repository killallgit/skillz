//! The workspace around its tabs: the tab bar, the key bar, help and toasts, and the keys that
//! belong to no tab.

use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::text::Line;

use std::sync::mpsc::Receiver;

use super::checks::Checks;
use super::frame;
use super::plugins::PluginsTab;
use super::run::RunModal;
use super::syncs::Syncs;
use super::tab::Tab;
use crate::app::review::ItemEvent;
use crate::app::workspace::Act;
use crate::app::workspace::Model;
use crate::app::workspace::{Screen, Synced};
use crate::tui::filter;
use crate::tui::help::{Help, Verdict};
use crate::tui::keymap::{self, Action, Scope};
use crate::tui::theme::Theme;
use crate::tui::widgets::container::{Container, Size};
use crate::tui::widgets::empty;
use crate::tui::widgets::keybar::KeyBar;
use crate::tui::widgets::margin;
use crate::tui::widgets::tabs::Tabs;
use crate::tui::widgets::toast::Toasts;
use skillz_harness::model::Scope as Level;

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
    syncs: Syncs,
    model: Model,
    /// The change under way, listed over the tab.
    run: Option<RunModal>,
}

impl Screen for Workspace {
    fn load(&mut self, mut model: Model) {
        if let Some(notice) = model.notice.take() {
            self.toasts.push(notice);
        }
        self.tabs.iter_mut().for_each(|tab| tab.load(&model));
        self.model = model;
    }

    fn watch_updates(&mut self, found: Receiver<String>) {
        self.checks.watch(found);
    }

    fn watch_syncs(&mut self, done: Receiver<Synced>) {
        self.syncs.watch(done);
    }

    fn forget_updates(&mut self, ids: &[String]) {
        self.checks.forget(ids);
    }
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
            syncs: Syncs::default(),
            model: Model::default(),
            run: None,
        }
    }

    /// Show where an item's change stands on every tab.
    pub fn note(&mut self, event: &ItemEvent) {
        if let Some(run) = &mut self.run {
            run.note(event);
        }
        self.tabs.iter_mut().for_each(|tab| tab.note(event));
    }

    /// Move the animations on a frame.
    pub fn tick(&mut self) {
        if let Some(run) = &mut self.run {
            run.tick();
        }
        self.tabs.iter_mut().for_each(|tab| tab.tick());
    }

    /// List `planned` over the workspace until `finish_run`.
    pub fn begin_run(&mut self, title: &str, planned: &[ItemEvent]) {
        self.run = Some(RunModal::new(title, planned));
    }

    pub fn finish_run(&mut self) {
        self.run = None;
    }

    /// The plugins list alone, for the project or for the user: with one tab there is no tab bar.
    pub fn plugins(scope: Level) -> Self {
        Workspace::new(vec![Box::new(PluginsTab::new(scope))])
    }

    fn take_syncs(&mut self) {
        for synced in self.syncs.finished() {
            if let Some(library) = synced.library {
                self.model.library = library;
                self.tabs.iter_mut().for_each(|tab| tab.load(&self.model));
            }
            if let Some(notice) = synced.notice {
                self.toasts.push(notice);
            }
        }
    }

    /// Tabs to switch between. A lone tab is shown bare: the list and nothing around it, with no
    /// frame, tab bar or key bar.
    fn has_bar(&self) -> bool {
        self.tabs.len() > 1
    }

    fn tab(&self) -> &dyn Tab {
        self.tabs[self.bar.selected()].as_ref()
    }

    fn tab_mut(&mut self) -> &mut dyn Tab {
        self.tabs[self.bar.selected()].as_mut()
    }

    fn scopes(&self) -> Vec<Scope> {
        let mut scopes = self.tab().scopes().to_vec();
        if self.has_bar() {
            scopes.push(Scope::Workspace);
        }
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
        self.take_syncs();
        let area = frame.area();
        if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
            return draw_too_small(frame, area, theme);
        }
        let inner = match self.has_bar() {
            true => frame::draw(frame, area, &self.model.dir, theme),
            false => margin::inset(area),
        };
        let head = u16::from(self.has_bar());
        // A bare tab has a footer only while its filter is being typed.
        let foot = u16::from(self.has_bar() || self.tab().prompt().is_some());
        let [bar, rule, body, foot] = Container::column([
            Size::Fixed(head),
            Size::Fixed(head),
            Size::Fill(1),
            Size::Fixed(foot),
        ])
        .areas(inner);
        let outdated = self.checks.outdated().clone();
        self.tabs
            .iter_mut()
            .for_each(|tab| tab.mark_outdated(&outdated));
        if self.has_bar() {
            self.bar.render(frame, frame::padded(bar), theme);
            frame::divider(frame, rule, theme);
        }
        self.tabs[self.bar.selected()].draw(frame, body, theme);
        if !foot.is_empty() {
            self.draw_footer(frame, frame::padded(foot), theme);
        }
        self.toasts.render(frame, inner, theme);
        if let Some(help) = &mut self.help {
            help.render(frame, theme);
        }
        if let Some(run) = &self.run {
            run.draw(frame, theme);
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
