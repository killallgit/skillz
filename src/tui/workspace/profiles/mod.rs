//! The Profiles tab: the named sets of library items, how each compares with the project, and
//! the highlighted profile's entries by kind. Profiles are built from the library, applied to
//! the project, and saved from what the project has.

mod chooser;
mod draw;
mod keys;
mod naming;
mod rows;
#[cfg(test)]
mod tests;

use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

use self::chooser::Chooser;
use self::naming::Naming;
use self::rows::Row;
use super::act::Act;
use super::model::Model;
use super::tab::Tab;
use crate::apply::compare;
use crate::library::Catalog;
use crate::profile::Profile;
use crate::tui::keymap::Scope;
use crate::tui::theme::Theme;
use crate::tui::widgets::confirm::ConfirmDialog;
use crate::tui::widgets::input::InputField;
use crate::tui::widgets::outline::Outline;

/// Which popup, if any, has the keys.
#[derive(Debug, Clone, Default)]
enum Popup {
    #[default]
    None,
    Filter,
    Name(Naming),
    Build(Box<Chooser>),
    /// Deleting the named profile, once confirmed.
    Delete(String, ConfirmDialog),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Focus {
    #[default]
    List,
    Entries,
}

pub struct ProfilesTab {
    profiles: Vec<Profile>,
    /// How many entries of each profile the project lacks, in the profiles' order.
    missing: Vec<usize>,
    library: Catalog,
    list: Outline,
    rows: Vec<Row>,
    entries: Outline,
    focus: Focus,
    popup: Popup,
}

impl ProfilesTab {
    pub fn new() -> Self {
        ProfilesTab {
            profiles: vec![],
            missing: vec![],
            library: Catalog::default(),
            list: Outline::new(vec![]),
            rows: vec![],
            entries: Outline::new(vec![]),
            focus: Focus::List,
            popup: Popup::None,
        }
    }

    /// The profile under the list cursor.
    fn current(&self) -> Option<&Profile> {
        self.list.current().map(|id| &self.profiles[id])
    }

    fn current_name(&self) -> Option<String> {
        self.current().map(|p| p.name.clone())
    }

    fn show_entries(&mut self) {
        let entries = self.current().map(|p| p.entries.clone());
        self.rows = rows::of_entries(&entries.unwrap_or_default());
        self.entries.reshape(rows::shape(&self.rows));
    }
}

impl Tab for ProfilesTab {
    fn title(&self) -> &'static str {
        "Profiles"
    }

    fn scopes(&self) -> &'static [Scope] {
        &[Scope::Profiles, Scope::Picker]
    }

    fn load(&mut self, model: &Model) {
        self.profiles = model.profiles.clone();
        self.missing = (self.profiles.iter())
            .map(|p| compare(&p.resolved(), &model.installed).missing.len())
            .collect();
        self.library = model.library.clone();
        let names = self.profiles.iter().map(|p| (None, p.name.clone()));
        self.list.reshape(names.collect());
        self.show_entries();
    }

    fn handle(&mut self, key: KeyEvent) -> Option<Act> {
        self.handle_key(key)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        self.draw_body(frame, area, theme);
    }

    fn captures_keys(&self) -> bool {
        !matches!(self.popup, Popup::None)
    }

    fn prompt(&self) -> Option<&InputField> {
        matches!(self.popup, Popup::Filter).then(|| self.list.field())
    }
}
