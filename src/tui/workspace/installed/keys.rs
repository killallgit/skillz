//! The keys of the Project and Global tabs.

use ratatui::crossterm::event::KeyEvent;

use super::picker::{Pick, Picker};
use super::rows::{Row, items_of};
use super::{InstalledTab, Popup};
use crate::tui::keymap::{self, Action};
use crate::tui::widgets::input::{InputField, Outcome as Edit};
use crate::tui::workspace::Tab;
use crate::tui::workspace::act::Act;

const NAME_PLACEHOLDER: &str = "profile name";
const SOURCE_PLACEHOLDER: &str = "owner/repo, git URL or path";

impl InstalledTab {
    pub(super) fn handle_key(&mut self, key: KeyEvent) -> Option<Act> {
        match std::mem::take(&mut self.popup) {
            Popup::None => self.handle_idle(key),
            Popup::Filter => self.handle_filter(key),
            Popup::Profiles(picker) => self.handle_picker(picker, key),
            Popup::Apply(picker) => self.handle_apply(picker, key),
            Popup::Save(field) => self.handle_save(field, key),
            Popup::Source(field) => self.handle_source(field, key),
        }
    }

    fn handle_idle(&mut self, key: KeyEvent) -> Option<Act> {
        match keymap::resolve_in(self.scopes(), key)? {
            Action::Filter => self.popup = Popup::Filter,
            Action::Back if !self.pending.is_empty() => self.pending.clear(),
            Action::Back if self.outline.is_filtered() => self.outline.clear_filter(),
            Action::PickProfile => self.open_picker(),
            Action::ApplyProfile => self.open_apply(),
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

    fn open_picker(&mut self) {
        let names = self.profiles.iter().map(|p| p.name.clone()).collect();
        self.popup = Popup::Profiles(Picker::new(names, self.compare.as_deref()));
    }

    fn open_apply(&mut self) {
        if !self.profiles.is_empty() {
            let names = self.profiles.iter().map(|p| p.name.clone()).collect();
            self.popup = Popup::Apply(Picker::applying(names));
        }
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

    /// The chosen profile replaces everything in the scope, after a plan to review.
    fn handle_apply(&mut self, mut picker: Picker, key: KeyEvent) -> Option<Act> {
        match picker.handle(key) {
            Pick::Open => self.popup = Popup::Apply(picker),
            Pick::Chosen(profile) => return profile.map(|profile| Act::ApplyExact { profile }),
            Pick::Closed => {}
        }
        None
    }

    fn handle_save(&mut self, mut field: InputField, key: KeyEvent) -> Option<Act> {
        match field.handle(key) {
            Edit::Editing => self.popup = Popup::Save(field),
            Edit::Submit => {
                let name = field.value().trim().to_string();
                return (!name.is_empty()).then_some(Act::SaveCurrent { name });
            }
            Edit::Cancel => {}
        }
        None
    }

    fn handle_picker(&mut self, mut picker: Picker, key: KeyEvent) -> Option<Act> {
        match picker.handle(key) {
            Pick::Open => self.popup = Popup::Profiles(picker),
            Pick::Chosen(profile) => self.compare = profile,
            Pick::Closed => {}
        }
        None
    }
}
