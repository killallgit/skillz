//! The workspace around the plugins list: help, toasts, the change under way, and the keys that
//! belong to no list.

use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::{Margin, Rect};
use ratatui::text::Line;

use std::sync::mpsc::Receiver;

use super::plugins::PluginsTab;
use super::run::RunModal;
use super::syncs::Syncs;
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
use crate::tui::widgets::margin;
use crate::tui::widgets::toast::Toasts;
use skillz_harness::model::Scope as Level;

#[cfg(test)]
mod tests;

const MIN_WIDTH: u16 = 40;
const MIN_HEIGHT: u16 = 10;

pub struct Workspace {
    list: PluginsTab,
    toasts: Toasts,
    help: Option<Help>,
    syncs: Syncs,
    model: Model,
    /// The change under way, listed over the list.
    run: Option<RunModal>,
}

impl Screen for Workspace {
    fn load(&mut self, mut model: Model) {
        if let Some(notice) = model.notice.take() {
            self.toasts.push(notice);
        }
        self.list.load(&model);
        self.model = model;
    }

    fn watch_syncs(&mut self, done: Receiver<Synced>) {
        self.syncs.watch(done);
    }
}

impl Workspace {
    /// The plugins list, for the project or for the user.
    pub fn plugins(scope: Level) -> Self {
        Workspace {
            list: PluginsTab::new(scope),
            toasts: Toasts::default(),
            help: None,
            syncs: Syncs::default(),
            model: Model::default(),
            run: None,
        }
    }

    /// Show where an item's change stands.
    pub fn note(&mut self, event: &ItemEvent) {
        if let Some(run) = &mut self.run {
            run.note(event);
        }
    }

    /// Move the animations on a frame.
    pub fn tick(&mut self) {
        if let Some(run) = &mut self.run {
            run.tick();
        }
    }

    /// List `planned` over the workspace until `finish_run`.
    pub fn begin_run(&mut self, title: &str, planned: &[ItemEvent]) {
        self.run = Some(RunModal::new(title, planned));
    }

    pub fn finish_run(&mut self) {
        self.run = None;
    }

    fn take_syncs(&mut self) {
        for synced in self.syncs.finished() {
            if let Some(library) = synced.library {
                self.model.library = library;
                self.list.load(&self.model);
            }
            if let Some(notice) = synced.notice {
                self.toasts.push(notice);
            }
        }
    }

    pub fn handle(&mut self, key: KeyEvent) -> Option<Act> {
        self.toasts.dismiss();
        if keymap::resolve(Scope::Common, key) == Some(Action::Interrupt) {
            return Some(Act::Quit);
        }
        if let Some(help) = self.help.take() {
            return self.handle_help(help, key);
        }
        if self.list.captures_keys() {
            return self.list.handle(key);
        }
        match keymap::resolve(Scope::Common, key) {
            Some(Action::Quit) => Some(Act::Quit),
            Some(Action::Help) => {
                self.help = Some(Help::new(self.list.scopes()));
                None
            }
            _ => self.list.handle(key),
        }
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
        let inner = margin::inset(area);
        // The list has a footer only while its filter is being typed.
        let foot = u16::from(self.list.prompt().is_some());
        let [body, foot] = Container::column([Size::Fill(1), Size::Fixed(foot)]).areas(inner);
        self.list.draw(frame, body, theme);
        if let Some(field) = self.list.prompt() {
            filter::draw_prompt(frame, foot.inner(Margin::new(1, 0)), field, theme);
        }
        self.toasts.render(frame, inner, theme);
        if let Some(help) = &mut self.help {
            help.render(frame, theme);
        }
        if let Some(run) = &self.run {
            run.draw(frame, theme);
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
