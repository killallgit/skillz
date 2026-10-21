//! The action menu Enter opens on a browsing tree.

use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::text::Line;

use crate::prompt::Choice;
use crate::tui::keymap::{self, Action, Scope};
use crate::tui::theme::Theme;
use crate::tui::widgets::list::ListView;
use crate::tui::widgets::modal::{self, ModalSize};
use crate::tui::widgets::panel::Panel;

/// Entries in display order; `None` closes the menu.
const ENTRIES: [(&str, Option<Choice>); 3] = [
    ("Show", Some(Choice::Show)),
    ("Remove…", Some(Choice::Remove)),
    ("Cancel", None),
];

/// What a key did to the menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pick {
    Open,
    Chosen(Choice),
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Menu {
    list: ListView,
}

impl Menu {
    pub fn new() -> Self {
        Menu {
            list: ListView::new(ENTRIES.len()),
        }
    }

    pub fn handle(&mut self, key: KeyEvent) -> Pick {
        match keymap::resolve(Scope::Common, key) {
            Some(Action::Select) => self.entry(),
            Some(Action::Back) => Pick::Closed,
            Some(movement) => {
                self.list.apply(movement);
                Pick::Open
            }
            None => Pick::Open,
        }
    }

    fn entry(&self) -> Pick {
        let at = self.list.selected().unwrap_or(0);
        ENTRIES[at].1.map_or(Pick::Closed, Pick::Chosen)
    }

    pub fn render(&mut self, frame: &mut Frame, theme: &Theme) {
        let size = ModalSize {
            width: 24,
            height: ENTRIES.len() as u16 + 2,
        };
        let inner = modal::open(frame, size, Panel::new("Do what?"), theme);
        let rows: Vec<Line<'static>> = ENTRIES.iter().map(|(l, _)| Line::from(*l)).collect();
        self.list.render(frame, inner, &rows, theme);
    }
}
