//! The popup that makes or edits a profile: its name, when it is new, above the library items
//! under their kinds with checkboxes, the ones the profile already has checked. One modal, one
//! save.

use std::collections::BTreeSet;

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::rows::{self, Row, section_title};
use crate::app::library::Catalog;
use crate::app::profile::{Profile, same_thing};
use crate::tui::keymap::{self, Action, Scope};
use crate::tui::theme::Theme;
use crate::tui::widgets::input::{InputField, Outcome};
use crate::tui::widgets::modal::{self, ModalSize};
use crate::tui::widgets::outline::Outline;
use crate::tui::widgets::panel::Panel;
use skillz_harness::Spec;

const SIZE: ModalSize = ModalSize {
    width: 70,
    height: 24,
};

/// What a key did to the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Choice {
    Open,
    /// Make the profile `name`, new or existing, hold exactly these entries.
    Save {
        name: String,
        entries: Vec<Spec>,
        new: bool,
    },
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Name,
    Items,
}

#[derive(Debug, Clone)]
pub(super) struct Editor {
    /// Typed by the user when the profile is new.
    name: Option<InputField>,
    profile: String,
    focus: Focus,
    /// Why the last save was refused, until the next key.
    refusal: Option<&'static str>,
    /// What the profile has that the library does not offer; kept as it is.
    kept: Vec<Spec>,
    rows: Vec<Row>,
    outline: Outline,
    checked: BTreeSet<usize>,
}

impl Editor {
    /// A new profile: named here, with nothing checked.
    pub fn creating(library: &Catalog) -> Self {
        Editor {
            name: Some(InputField::new("name")),
            focus: Focus::Name,
            ..Self::editing(&Profile::new(""), library)
        }
    }

    pub fn editing(profile: &Profile, library: &Catalog) -> Self {
        let rows = rows::of_library(library);
        let has = |entry: &Spec| profile.entries.iter().any(|e| same_thing(e, entry));
        let checked = (0..rows.len())
            .filter(|&id| matches!(&rows[id], Row::Item(item) if has(&item.entry)))
            .collect();
        let offered = |entry: &Spec| library.items().iter().any(|i| same_thing(&i.entry, entry));
        let mut outline = Outline::new(rows::shape(&rows));
        outline.open_all();
        Editor {
            name: None,
            profile: profile.name.clone(),
            focus: Focus::Items,
            refusal: None,
            kept: profile
                .entries
                .iter()
                .filter(|e| !offered(e))
                .cloned()
                .collect(),
            outline,
            rows,
            checked,
        }
    }

    /// Keep the editor open and say why the save did not happen.
    pub fn refuse(&mut self, reason: &'static str) {
        self.refusal = Some(reason);
        self.focus = Focus::Name;
    }

    pub fn handle(&mut self, key: KeyEvent) -> Choice {
        self.refusal = None;
        match self.focus {
            Focus::Name => self.handle_name(key),
            Focus::Items => self.handle_items(key),
        }
    }

    fn handle_name(&mut self, key: KeyEvent) -> Choice {
        if matches!(key.code, KeyCode::Tab | KeyCode::Down) {
            self.focus = Focus::Items;
            return Choice::Open;
        }
        let Some(field) = self.name.as_mut() else {
            return Choice::Open;
        };
        match field.handle(key) {
            Outcome::Editing => Choice::Open,
            Outcome::Submit => self.save(),
            Outcome::Cancel => Choice::Closed,
        }
    }

    fn handle_items(&mut self, key: KeyEvent) -> Choice {
        let plain = key.modifiers.is_empty();
        match (key.code, plain) {
            (KeyCode::Char('s'), true) => return self.save(),
            (KeyCode::Tab | KeyCode::BackTab, _) if self.name.is_some() => {
                self.focus = Focus::Name;
                return Choice::Open;
            }
            (KeyCode::Up, _) if self.name.is_some() && self.outline.current() == Some(0) => {
                self.focus = Focus::Name;
                return Choice::Open;
            }
            _ => {}
        }
        match keymap::resolve(Scope::Picker, key) {
            Some(Action::Select) => self.save(),
            Some(Action::Back) => Choice::Closed,
            Some(Action::Toggle) => {
                self.toggle_current();
                Choice::Open
            }
            Some(movement) => {
                self.outline.navigate(movement);
                Choice::Open
            }
            None => Choice::Open,
        }
    }

    fn save(&mut self) -> Choice {
        let name = match &self.name {
            Some(field) => field.value().trim().to_string(),
            None => self.profile.clone(),
        };
        if name.is_empty() {
            self.focus = Focus::Name;
            return Choice::Open;
        }
        Choice::Save {
            name,
            entries: self.entries(),
            new: self.name.is_some(),
        }
    }

    fn toggle_current(&mut self) {
        let Some(id) = self.outline.current() else {
            return;
        };
        match self.rows[id] {
            Row::Section { .. } => self.outline.navigate(Action::Toggle),
            Row::Item(_) => {
                if !self.checked.remove(&id) {
                    self.checked.insert(id);
                }
            }
        }
    }

    /// The kept entries, then the checked library ones.
    fn entries(&self) -> Vec<Spec> {
        let chosen = self.checked.iter().filter_map(|&id| match &self.rows[id] {
            Row::Item(item) => Some(item.entry.clone()),
            Row::Section { .. } => None,
        });
        self.kept.iter().cloned().chain(chosen).collect()
    }

    pub fn render(&mut self, frame: &mut Frame, theme: &Theme) {
        let title = match &self.name {
            Some(_) => "New profile".to_string(),
            None => format!("Edit {}", self.profile),
        };
        let panel = Panel::new(title).badge(self.outline.badge());
        let inner = modal::open(frame, SIZE, panel, theme);
        let name_height = u16::from(self.name.is_some()) * 2;
        let [name, list, foot] = Layout::vertical([
            Constraint::Length(name_height),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .areas(inner);
        if let Some(field) = &self.name {
            let [label, input] =
                Layout::horizontal([Constraint::Length(6), Constraint::Min(1)]).areas(name);
            let style = match self.focus {
                Focus::Name => theme.accent,
                Focus::Items => theme.muted,
            };
            frame.render_widget(Paragraph::new(Span::styled("Name", style)), label);
            if self.focus == Focus::Name {
                field.render(frame, input, theme);
            } else {
                frame.render_widget(Paragraph::new(field.value().to_string()), input);
            }
        }
        let lines: Vec<Line<'static>> = self
            .outline
            .visible()
            .iter()
            .map(|&id| self.line(id, theme))
            .collect();
        self.outline.render(frame, list, &lines, theme);
        frame.render_widget(Paragraph::new(self.hints(theme)), foot);
    }

    fn hints(&self, theme: &Theme) -> Line<'static> {
        if let Some(reason) = self.refusal {
            return Line::styled(reason, theme.warn);
        }
        let mut hints = vec!["space check", "s save", "esc cancel"];
        if self.name.is_some() {
            hints.insert(0, "tab name/list");
        }
        Line::styled(hints.join("  "), theme.muted)
    }

    fn line(&self, id: usize, theme: &Theme) -> Line<'static> {
        match &self.rows[id] {
            Row::Section { kind, count } => {
                let caret = self.outline.caret(id, &theme.glyphs);
                Line::from(vec![
                    Span::styled(format!("{caret} {}", section_title(*kind)), theme.title),
                    Span::styled(format!("  {count}"), theme.muted),
                ])
            }
            Row::Item(item) => {
                let glyphs = &theme.glyphs;
                let (mark, style) = match self.checked.contains(&id) {
                    true => (glyphs.checked, theme.ok),
                    false => (glyphs.unchecked, theme.muted),
                };
                Line::from(vec![
                    Span::styled(format!("  {mark} "), style),
                    Span::styled(item.label.clone(), theme.text),
                ])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::KeyCode;

    use super::rows::tests::skill;
    use super::*;
    use crate::app::library::LibraryItem;
    use crate::tui::widgets::testing::key;

    fn library() -> Catalog {
        Catalog::from_items(vec![
            LibraryItem::from_entry(skill("a")),
            LibraryItem::from_entry(skill("b")),
        ])
    }

    fn profile_with(entries: Vec<Spec>) -> Profile {
        let mut profile = Profile::new("web");
        profile.entries = entries;
        profile
    }

    fn press(chooser: &mut Editor, codes: &[KeyCode]) -> Choice {
        codes
            .iter()
            .fold(Choice::Open, |_, code| chooser.handle(key(*code)))
    }

    fn saved(entries: Vec<Spec>) -> Choice {
        Choice::Save {
            name: "web".into(),
            entries,
            new: false,
        }
    }

    const SPACE: KeyCode = KeyCode::Char(' ');

    #[test]
    fn items_the_profile_has_start_checked_and_saving_keeps_them() {
        let mut chooser = Editor::editing(&profile_with(vec![skill("a")]), &library());
        chooser.outline.open_all();

        let choice = press(&mut chooser, &[KeyCode::Enter]);

        assert_eq!(choice, saved(vec![skill("a")]));
    }

    #[test]
    fn space_checks_and_unchecks_the_item_under_the_cursor() {
        let mut chooser = Editor::editing(&profile_with(vec![skill("a")]), &library());
        chooser.outline.open_all();

        // Section, a, b: check b, then uncheck a.
        press(&mut chooser, &[KeyCode::Down, KeyCode::Down, SPACE]);
        press(&mut chooser, &[KeyCode::Up, SPACE]);
        let choice = press(&mut chooser, &[KeyCode::Enter]);

        assert_eq!(choice, saved(vec![skill("b")]));
    }

    #[test]
    fn entries_the_library_does_not_offer_are_kept() {
        let mine = skill("mine");
        let mut chooser = Editor::editing(&profile_with(vec![mine.clone()]), &library());
        chooser.outline.open_all();

        press(&mut chooser, &[KeyCode::Down, SPACE]);
        let choice = press(&mut chooser, &[KeyCode::Enter]);

        assert_eq!(choice, saved(vec![mine, skill("a")]));
    }

    #[test]
    fn space_on_a_section_collapses_it_without_checking_anything() {
        let mut chooser = Editor::editing(&profile_with(vec![]), &library());
        chooser.outline.open_all();

        press(&mut chooser, &[SPACE]);

        assert_eq!(chooser.outline.visible().len(), 1);
        assert_eq!(press(&mut chooser, &[KeyCode::Enter]), saved(vec![]));
    }

    #[test]
    fn escape_closes_without_a_result() {
        let mut chooser = Editor::editing(&profile_with(vec![]), &library());
        chooser.outline.open_all();

        assert_eq!(press(&mut chooser, &[KeyCode::Esc]), Choice::Closed);
    }
}
