//! The Profiles tab: one list of the named sets of library items, each with how it compares
//! with the project and its entries beneath it. Profiles are made and edited in one modal,
//! applied to the project, and saved from what the project has.

mod draw;
mod editor;
mod keys;
mod rows;
#[cfg(test)]
mod tests;

use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

use self::editor::Editor;
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
    Edit(Box<Editor>),
    /// Deleting the named profile, once confirmed.
    Delete(String, ConfirmDialog),
}

struct ProfileRow {
    profile: Profile,
    missing: usize,
}

pub struct ProfilesTab {
    profiles: Vec<ProfileRow>,
    library: Catalog,
    list: Outline,
    list_rows: Vec<rows::ListRow>,
    popup: Popup,
}

impl ProfilesTab {
    pub fn new() -> Self {
        ProfilesTab {
            profiles: vec![],
            library: Catalog::default(),
            list: Outline::new(vec![]),
            list_rows: vec![],
            popup: Popup::None,
        }
    }

    /// The profile under the cursor, or the one whose entry is.
    fn current(&self) -> Option<&Profile> {
        let id = self.list.current()?;
        let at = self.list_rows.get(id)?.profile();
        Some(&self.profiles[at].profile)
    }

    fn current_name(&self) -> Option<String> {
        self.current().map(|p| p.name.clone())
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
        self.profiles = model
            .profiles
            .iter()
            .map(|profile| ProfileRow {
                missing: compare(&profile.resolved(), &model.installed).missing.len(),
                profile: profile.clone(),
            })
            .collect();
        self.library = model.library.clone();
        self.list_rows = rows::of_profiles(self.profiles.iter().map(|row| &row.profile));
        let names: Vec<String> = self
            .profiles
            .iter()
            .map(|r| r.profile.name.clone())
            .collect();
        self.list.reshape(rows::list_shape(&self.list_rows, &names));
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
