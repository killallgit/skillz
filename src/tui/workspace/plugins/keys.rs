//! The plugins list's keys.

use ratatui::crossterm::event::KeyEvent;

use super::Info;
use super::PluginsTab;
use super::rows::Row;
use crate::app::workspace::Act;
use crate::tui::keymap::{self, Action, Scope};
use crate::tui::widgets::input::Outcome as Edit;
use crate::tui::widgets::text_modal::TextModal;
use crate::tui::workspace::Tab;
use crate::tui::workspace::tools::for_tools;

impl PluginsTab {
    pub(super) fn handle_key(&mut self, key: KeyEvent) -> Option<Act> {
        if let Some(info) = &mut self.info {
            match keymap::resolve_in(&[Scope::Plugins], key) {
                Some(Action::Back | Action::Select | Action::Quit | Action::Info) => {
                    self.info = None;
                }
                Some(movement) => info.modal.navigate(movement),
                None => {}
            }
            return None;
        }
        if self.filtering {
            self.filtering = self.outline.filter_key(key) == Edit::Editing;
            return None;
        }
        match keymap::resolve_in(self.scopes(), key)? {
            Action::Filter => self.filtering = true,
            Action::Back if !self.pending.is_empty() => self.pending.clear(),
            Action::Back if self.outline.is_filtered() => self.outline.clear_filter(),
            Action::Info => self.show_info(),
            Action::Toggle => self.select(),
            Action::Select => return self.apply(),
            movement => {
                self.outline.navigate(movement);
                self.settle(movement);
            }
        }
        None
    }

    /// Move the cursor off the gap and the divider, further the way `moved` was going, else back.
    pub(super) fn settle(&mut self, moved: Action) {
        let (onward, back) = match moved {
            Action::Up | Action::PageUp | Action::HalfPageUp => (Action::Up, Action::Down),
            _ => (Action::Down, Action::Up),
        };
        for direction in [onward, back] {
            while self.on_inert_row() {
                let before = self.outline.current();
                self.outline.navigate(direction);
                if self.outline.current() == before {
                    break;
                }
            }
        }
    }

    fn on_inert_row(&self) -> bool {
        self.outline
            .current()
            .is_some_and(|id| self.rows[id].is_inert())
    }

    /// Open the info modal for the package under the cursor.
    fn show_info(&mut self) {
        let Some(id) = self.outline.current() else {
            return;
        };
        if let Row::Own(plugin) | Row::Global(plugin) = &self.rows[id] {
            self.info = Some(Info {
                title: plugin.name(),
                lines: plugin.info.clone(),
                modal: TextModal::default(),
            });
        }
    }

    /// Flip the package under the cursor between staying as it is and changing. The user's
    /// packages and the divider above them are left alone.
    fn select(&mut self) {
        let Some(id) = self.outline.current() else {
            return;
        };
        let Row::Own(plugin) = &self.rows[id] else {
            return;
        };
        let (package, installed) = (plugin.package.clone(), plugin.is_installed());
        // A package with nothing the library could install can only be removed.
        if !installed && plugin.entries.is_empty() {
            return;
        }
        if self.pending.remove(&package).is_none() {
            self.pending.insert(package, installed);
        }
    }

    /// Everything flipped as one change: each package that was installed is removed, each that
    /// was not is installed for every tool, keeping an entry that names its own tools to those.
    fn apply(&self) -> Option<Act> {
        let (mut install, mut remove) = (Vec::new(), Vec::new());
        for row in &self.rows {
            let Row::Own(plugin) = row else { continue };
            if !self.pending.contains_key(&plugin.package) {
                continue;
            }
            if plugin.is_installed() {
                remove.extend(plugin.installed.iter().cloned());
            } else {
                install.extend(
                    plugin
                        .entries
                        .iter()
                        .cloned()
                        .filter_map(|entry| for_tools(entry, &self.agents, true)),
                );
            }
        }
        (!install.is_empty() || !remove.is_empty()).then_some(Act::Change {
            scope: self.scope,
            install,
            remove,
        })
    }
}
