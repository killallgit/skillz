//! Moving through an outline and editing its filter.

use ratatui::crossterm::event::KeyEvent;

use super::Outline;
use crate::tui::keymap::{self, Action, Scope};
use crate::tui::widgets::input::Outcome as Edit;

impl Outline {
    /// Expand, collapse or move the cursor as `action` says; anything else is ignored.
    pub fn navigate(&mut self, action: Action) {
        let Some(id) = self.current() else { return };
        match action {
            Action::Right if self.has_children[id] && !self.expanded.contains(&id) => {
                self.toggle_open(id);
            }
            Action::Toggle if self.has_children[id] => self.toggle_open(id),
            Action::Left => self.collapse_or_ascend(id),
            movement => {
                self.list.apply(movement);
            }
        }
    }

    fn collapse_or_ascend(&mut self, id: usize) {
        if self.expanded.contains(&id) {
            self.toggle_open(id);
        } else if let Some(parent) = self.parents[id] {
            self.select_row(parent);
        }
    }

    /// Navigation stays in the list; all other keys go to the filter field.
    pub fn filter_key(&mut self, key: KeyEvent) -> Edit {
        if let Some(movement) = keymap::resolve(Scope::Filter, key) {
            self.list.apply(movement);
            return Edit::Editing;
        }
        self.edit_filter(key)
    }

    /// Feed a key to the filter field. Typing re-filters from the top; cancelling clears it.
    fn edit_filter(&mut self, key: KeyEvent) -> Edit {
        let before = self.filter.value().to_string();
        let outcome = self.filter.handle(key);
        match outcome {
            Edit::Cancel => self.clear_filter(),
            _ if self.filter.value() != before => {
                self.list.select(0);
                self.refresh();
            }
            _ => {}
        }
        outcome
    }
}
