//! The Project tab: what the directory has installed, by kind, and the library items it could
//! have. Nothing is stored here: the rows are derived from the agents' own config and the
//! library each time the workspace loads a new snapshot.

mod direct;
#[cfg(test)]
mod direct_tests;
mod draw;
mod drift;
mod header;
mod keys;
mod location;
mod pending;
mod picker;
#[cfg(test)]
mod picking_tests;
mod presence;
mod rows;
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
use self::drift::{Drift, drift};
use self::pending::Pending;
use self::picker::Picker;
use self::rows::{Row, Sources, build, shape};
use super::act::Act;
use super::model::Model;
use super::tab::Tab;
use crate::model::Augmentation;
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
    Profiles(Picker),
    AlsoGlobal(ConfirmDialog, Proposal),
}

pub struct ProjectTab {
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
}

impl ProjectTab {
    pub fn new() -> Self {
        ProjectTab {
            dir: String::new(),
            installed: vec![],
            profiles: vec![],
            rows: vec![],
            outline: Outline::new(vec![]),
            pending: Pending::default(),
            compare: None,
            popup: Popup::None,
            outdated: BTreeSet::new(),
        }
    }

    fn drift(&self) -> Option<Drift> {
        let name = self.compare.as_deref()?;
        let profile = self.profiles.iter().find(|p| p.name == name)?;
        Some(drift(profile, &self.installed))
    }
}

impl Tab for ProjectTab {
    fn title(&self) -> &'static str {
        "Project"
    }

    fn scopes(&self) -> &'static [Scope] {
        &[Scope::Project]
    }

    fn load(&mut self, model: &Model) {
        self.dir = model.dir.clone();
        self.installed = model.installed.clone();
        self.profiles = model.profiles.clone();
        self.rows = build(&Sources {
            project: &model.installed,
            global: &model.global,
            library: &model.library,
        });
        self.pending.keep_existing(&self.rows);
        self.outline.reshape(shape(&self.rows));
    }

    fn mark_outdated(&mut self, ids: &BTreeSet<String>) {
        self.outdated.clone_from(ids);
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
        matches!(self.popup, Popup::Filter).then(|| self.outline.field())
    }
}
