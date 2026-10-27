//! The Project and Global tabs, one implementation over a scope: what the scope has installed,
//! by kind, as whole units. Nothing is stored here: the rows are derived from the agents' own
//! config each time the workspace loads a new snapshot.

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
mod rows;
mod status;
#[cfg(test)]
mod status_tests;
mod subset;
#[cfg(test)]
mod tests;
mod update;
#[cfg(test)]
mod update_tests;

use std::collections::BTreeSet;

use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

use self::pending::Pending;
use self::picker::Picker;
use self::rows::{Row, build, shape};
use self::status::Statuses;
use self::subset::Subset;
use super::tab::Tab;
use crate::app::apply::{Drift, drift};
use crate::app::profile::Profile;
use crate::app::review::ItemEvent;
use crate::app::workspace::Act;
use crate::app::workspace::Model;
use crate::tui::keymap;
use crate::tui::theme::Theme;
use crate::tui::widgets::input::InputField;
use crate::tui::widgets::outline::Outline;
use skillz_harness::model::{Installed, Kind, Scope};

/// Which popup, if any, has the keys.
#[derive(Debug, Clone, Default)]
enum Popup {
    #[default]
    None,
    Filter,
    /// Choosing the profile to apply, and how.
    Apply(Picker),
    /// Choosing which entries of the applied profile the project keeps.
    Subset(Subset),
    /// Typing the name to save what is installed as a profile.
    Save(InputField),
    /// Typing the repo or path to install from.
    Source(InputField),
}

pub struct InstalledTab {
    scope: Scope,
    dir: String,
    installed: Vec<Installed>,
    profiles: Vec<Profile>,
    rows: Vec<Row>,
    outline: Outline,
    pending: Pending,
    /// The profile last applied to the project, which the header compares it with.
    applied: Option<String>,
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
        crate::tui::widgets::testing::open_all(&mut self.outline);
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
            applied: None,
            popup: Popup::None,
            outdated: BTreeSet::new(),
            statuses: Statuses::default(),
        }
    }

    fn applied_profile(&self) -> Option<&Profile> {
        let name = self.applied.as_deref()?;
        self.profiles.iter().find(|p| p.name == name)
    }

    fn drift(&self) -> Option<Drift> {
        Some(drift(self.applied_profile()?, &self.installed))
    }

    fn read_only(&self, row: usize) -> bool {
        std::iter::successors(Some(row), |&id| self.outline.parent(id))
            .any(|id| matches!(self.rows[id], Row::Inherited))
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
        let here = match self.scope {
            Scope::Project => &model.installed,
            Scope::Global => &model.global,
        };
        self.installed = here.clone();
        self.profiles = model.profiles.clone();
        let applied = match self.scope {
            Scope::Project => &model.applied,
            Scope::Global => &model.applied_global,
        };
        self.applied.clone_from(applied);
        self.rows = build(here);
        if self.scope == Scope::Project {
            let inherited: Vec<_> = model
                .global
                .iter()
                .filter(|item| matches!(item.kind, Kind::Plugin | Kind::Skill))
                .cloned()
                .collect();
            if !inherited.is_empty() {
                self.rows.push(Row::Inherited);
                self.rows.extend(build(&inherited));
            }
        }
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
