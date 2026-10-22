//! The Project and Global tabs, one implementation over a scope: what the scope has installed,
//! by kind, and the library items it could have. Nothing is stored here: the rows are derived
//! from the agents' own config and the library each time the workspace loads a new snapshot.

mod direct;
#[cfg(test)]
mod direct_tests;
mod draw;
mod draw_group;
#[cfg(test)]
mod global_tests;
mod header;
mod keys;
mod pending;
mod picker;
#[cfg(test)]
mod picking_tests;
mod presence;
mod rows;
mod status;
#[cfg(test)]
mod status_tests;
#[cfg(test)]
mod tests;
mod update;
#[cfg(test)]
mod update_tests;

use std::collections::BTreeSet;

use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

use self::direct::Proposal;
use self::pending::Pending;
use self::picker::Picker;
use self::rows::{Row, Sources, build, shape};
use self::status::Statuses;
use super::act::Act;
use super::model::Model;
use super::tab::Tab;
use crate::apply::{Drift, drift};
use crate::model::{Augmentation, Scope};
use crate::profile::Profile;
use crate::review::ItemEvent;
use crate::tui::keymap;
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
    Profiles(Picker),
    AlsoGlobal(ConfirmDialog, Proposal),
}

pub struct InstalledTab {
    scope: Scope,
    dir: String,
    installed: Vec<Augmentation>,
    profiles: Vec<Profile>,
    rows: Vec<Row>,
    outline: Outline,
    pending: Pending,
    /// The profile the header compares the project with.
    compare: Option<String>,
    popup: Popup,
    /// Ids of installed items the background check found newer versions of.
    outdated: BTreeSet<String>,
    statuses: Statuses,
}

impl InstalledTab {
    /// Load a snapshot and open every group, so a test can walk the rows.
    #[cfg(test)]
    pub(super) fn load_open(&mut self, model: &Model) {
        self.load(model);
        self.outline.open_all();
    }

    pub fn project() -> Self {
        Self::new(Scope::Project)
    }

    pub fn global() -> Self {
        Self::new(Scope::Global)
    }

    fn new(scope: Scope) -> Self {
        InstalledTab {
            scope,
            dir: String::new(),
            installed: vec![],
            profiles: vec![],
            rows: vec![],
            outline: Outline::new(vec![]),
            pending: Pending::default(),
            compare: None,
            popup: Popup::None,
            outdated: BTreeSet::new(),
            statuses: Statuses::default(),
        }
    }

    fn drift(&self) -> Option<Drift> {
        let name = self.compare.as_deref()?;
        let profile = self.profiles.iter().find(|p| p.name == name)?;
        Some(drift(profile, &self.installed))
    }
}

impl Tab for InstalledTab {
    fn title(&self) -> &'static str {
        match self.scope {
            Scope::Project => "Project",
            Scope::Global => "Global",
        }
    }

    fn scopes(&self) -> &'static [keymap::Scope] {
        match self.scope {
            Scope::Project => &[keymap::Scope::Installed, keymap::Scope::Project],
            Scope::Global => &[keymap::Scope::Installed],
        }
    }

    fn load(&mut self, model: &Model) {
        self.dir = model.dir.clone();
        let (here, also_global) = match self.scope {
            Scope::Project => (&model.installed, &model.global[..]),
            Scope::Global => (&model.global, &[][..]),
        };
        self.installed = here.clone();
        self.profiles = model.profiles.clone();
        self.rows = build(&Sources {
            here,
            also_global,
            library: &model.library,
        });
        self.pending.keep_existing(&self.rows);
        self.statuses.clear();
        self.outline.reshape(shape(&self.rows));
    }

    fn mark_outdated(&mut self, ids: &BTreeSet<String>) {
        self.outdated.clone_from(ids);
    }

    fn handle(&mut self, key: KeyEvent) -> Option<Act> {
        self.statuses.clear();
        self.handle_key(key)
    }

    fn note(&mut self, event: &ItemEvent) {
        self.statuses.note(event);
    }

    fn tick(&mut self) {
        self.statuses.tick();
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        self.draw_body(frame, area, theme);
    }

    fn captures_keys(&self) -> bool {
        !matches!(self.popup, Popup::None)
    }

    fn prompt(&self) -> Option<&InputField> {
        matches!(self.popup, Popup::Filter).then(|| self.outline.field())
    }
}
