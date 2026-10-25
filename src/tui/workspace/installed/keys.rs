//! The keys of the Project and Global tabs.

use ratatui::crossterm::event::KeyEvent;

use super::picker::{Pick, Picker};
use super::rows::{Row, items_of};
use super::subset::{Choice, Subset};
use super::{InstalledTab, Popup};
use crate::app::workspace::Act;
use crate::tui::keymap::{self, Action};
use crate::tui::widgets::input::{InputField, Outcome as Edit};
use crate::tui::workspace::Tab;

const NAME_PLACEHOLDER: &str = "profile name";
const SOURCE_PLACEHOLDER: &str = "owner/repo, git URL or path";

impl InstalledTab {
    pub(super) fn handle_key(&mut self, key: KeyEvent) -> Option<Act> {
        match std::mem::take(&mut self.popup) {
            Popup::None => self.handle_idle(key),
            Popup::Filter => self.handle_filter(key),
            Popup::Apply(picker) => self.handle_apply(picker, key),
            Popup::Subset(subset) => self.handle_subset(subset, key),
            Popup::Save(field) => self.handle_save(field, key),
            Popup::Source(field) => self.handle_source(field, key),
        }
    }

    fn handle_idle(&mut self, key: KeyEvent) -> Option<Act> {
        match keymap::resolve_in(self.scopes(), key)? {
            Action::Filter => self.popup = Popup::Filter,
            Action::Back if !self.pending.is_empty() => self.pending.clear(),
            Action::Back if self.outline.is_filtered() => self.outline.clear_filter(),
            Action::ApplyProfile => self.open_apply(),
            Action::PickSubset => self.open_subset(),
            Action::Clear => return self.clear(),
            Action::SaveCurrent => self.popup = Popup::Save(InputField::new(NAME_PLACEHOLDER)),
            Action::Update => return self.update(),
            Action::AddSource => self.popup = Popup::Source(InputField::new(SOURCE_PLACEHOLDER)),
            Action::Remove => return self.remove_current(),
            Action::Toggle => self.toggle(),
            Action::Select => return self.select(),
            movement => self.outline.navigate(movement),
        }
        None
    }

    /// Update the highlighted installed item, or the installed items of the highlighted section.
    /// Anything that cannot be updated is left alone without a word.
    fn update(&self) -> Option<Act> {
        let ids = self.updatable_ids();
        let scope = self.scope;
        (!ids.is_empty()).then_some(Act::Update { scope, ids })
    }

    fn open_apply(&mut self) {
        if !self.profiles.is_empty() {
            let names = self.profiles.iter().map(|p| p.name.clone()).collect();
            self.popup = Popup::Apply(Picker::new(names, self.applied.as_deref()));
        }
    }

    fn open_subset(&mut self) {
        if let Some(profile) = self.applied_profile() {
            self.popup = Popup::Subset(Subset::of(profile, &self.installed));
        }
    }

    /// Remove everything the project has, as one plan.
    fn clear(&self) -> Option<Act> {
        let ids: Vec<String> = self
            .rows
            .iter()
            .filter_map(|row| match row {
                Row::Item(item) => Some(item.id.clone()),
                _ => None,
            })
            .collect();
        (!ids.is_empty()).then_some(Act::Remove {
            scope: self.scope,
            ids,
        })
    }

    fn toggle(&mut self) {
        match self.acted_on().as_slice() {
            [] => self.fold(),
            ids => self.pending.toggle_all(ids),
        }
    }

    /// Apply what is picked; with nothing picked, a section opens or closes.
    fn select(&mut self) -> Option<Act> {
        if self.pending.is_empty() {
            self.fold();
            return None;
        }
        let scope = self.scope;
        Some(Act::Remove {
            scope,
            ids: self.pending.picks(),
        })
    }

    /// The ids of the highlighted item, or of the items in the highlighted package. A section
    /// is no unit to act on.
    pub(super) fn acted_on(&self) -> Vec<String> {
        let Some(at) = self.outline.current() else {
            return vec![];
        };
        match &self.rows[at] {
            Row::Item(item) => vec![item.id.clone()],
            Row::Package(_) => items_of(&self.rows, at)
                .map(|item| item.id.clone())
                .collect(),
            Row::Section(_) => vec![],
        }
    }

    /// The ids of every item in the highlighted section, when a section is highlighted.
    pub(super) fn section_ids(&self) -> Option<Vec<String>> {
        let at = self.outline.current()?;
        matches!(self.rows[at], Row::Section(_)).then(|| {
            items_of(&self.rows, at)
                .map(|item| item.id.clone())
                .collect()
        })
    }

    fn fold(&mut self) {
        if let Some(id) = self.outline.current() {
            self.outline.toggle_open(id);
        }
    }

    fn handle_filter(&mut self, key: KeyEvent) -> Option<Act> {
        self.popup = if self.outline.filter_key(key) == Edit::Editing {
            Popup::Filter
        } else {
            Popup::None
        };
        None
    }

    fn handle_source(&mut self, mut field: InputField, key: KeyEvent) -> Option<Act> {
        match field.handle(key) {
            Edit::Editing => self.popup = Popup::Source(field),
            Edit::Submit => {
                let source = field.value().trim().to_string();
                let scope = self.scope;
                return (!source.is_empty()).then_some(Act::InstallSource { scope, source });
            }
            Edit::Cancel => {}
        }
        None
    }

    /// The chosen profile is applied, after a plan to review.
    fn handle_apply(&mut self, mut picker: Picker, key: KeyEvent) -> Option<Act> {
        match picker.handle(key) {
            Pick::Open => self.popup = Popup::Apply(picker),
            Pick::Chosen {
                profile,
                exact: true,
            } => return Some(Act::ApplyExact { profile }),
            Pick::Chosen {
                profile,
                exact: false,
            } => return Some(Act::Apply { profile }),
            Pick::Closed => {}
        }
        None
    }

    fn handle_subset(&mut self, mut subset: Subset, key: KeyEvent) -> Option<Act> {
        match subset.handle(key, &self.installed) {
            Choice::Open => self.popup = Popup::Subset(subset),
            Choice::Done(act) => return Some(act),
            Choice::Closed => {}
        }
        None
    }

    fn handle_save(&mut self, mut field: InputField, key: KeyEvent) -> Option<Act> {
        match field.handle(key) {
            Edit::Editing => self.popup = Popup::Save(field),
            Edit::Submit => {
                let name = field.value().trim().to_string();
                if name.is_empty() {
                    self.popup = Popup::Save(field);
                    return None;
                }
                return Some(Act::SaveCurrent { name });
            }
            Edit::Cancel => {}
        }
        None
    }
}
