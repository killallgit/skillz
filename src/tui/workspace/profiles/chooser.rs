//! The popup that builds a profile: library items under their kinds with checkboxes, the ones
//! the profile already has checked.

use std::collections::BTreeSet;

use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::text::{Line, Span};

use super::rows::{self, Row, section_title};
use crate::library::Catalog;
use crate::profile::{Entry, Profile, same_thing};
use crate::tui::keymap::{self, Action, Scope};
use crate::tui::theme::Theme;
use crate::tui::widgets::modal::{self, ModalSize};
use crate::tui::widgets::outline::Outline;
use crate::tui::widgets::panel::Panel;

const SIZE: ModalSize = ModalSize {
    width: 70,
    height: 22,
};

/// What a key did to the chooser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Choice {
    Open,
    /// The profile's new entries.
    Done(Vec<Entry>),
    Closed,
}

#[derive(Debug, Clone)]
pub(super) struct Chooser {
    profile: String,
    /// What the profile has that the library does not offer; kept as it is.
    kept: Vec<Entry>,
    rows: Vec<Row>,
    outline: Outline,
    checked: BTreeSet<usize>,
}

impl Chooser {
    pub fn new(profile: &Profile, library: &Catalog) -> Self {
        let rows = rows::of_library(library);
        let has = |entry: &Entry| profile.entries.iter().any(|e| same_thing(e, entry));
        let checked = (0..rows.len())
            .filter(|&id| matches!(&rows[id], Row::Item(item) if has(&item.entry)))
            .collect();
        let offered = |entry: &Entry| library.items().iter().any(|i| same_thing(&i.entry, entry));
        Chooser {
            profile: profile.name.clone(),
            kept: profile
                .entries
                .iter()
                .filter(|e| !offered(e))
                .cloned()
                .collect(),
            outline: rows::closed_outline(&rows),
            rows,
            checked,
        }
    }

    pub fn profile(&self) -> &str {
        &self.profile
    }

    pub fn handle(&mut self, key: KeyEvent) -> Choice {
        match keymap::resolve(Scope::Picker, key) {
            Some(Action::Select) => Choice::Done(self.entries()),
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
    fn entries(&self) -> Vec<Entry> {
        let chosen = self.checked.iter().filter_map(|&id| match &self.rows[id] {
            Row::Item(item) => Some(item.entry.clone()),
            Row::Section { .. } => None,
        });
        self.kept.iter().cloned().chain(chosen).collect()
    }

    pub fn render(&mut self, frame: &mut Frame, theme: &Theme) {
        let panel = Panel::new(format!("Build {}", self.profile)).badge(self.outline.badge());
        let inner = modal::open(frame, SIZE, panel, theme);
        let lines: Vec<Line<'static>> = self
            .outline
            .visible()
            .iter()
            .map(|&id| self.line(id, theme))
            .collect();
        self.outline.render(frame, inner, &lines, theme);
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
    use crate::library::LibraryItem;
    use crate::tui::widgets::testing::key;

    fn library() -> Catalog {
        Catalog::from_items(vec![
            LibraryItem::from_entry(skill("a")),
            LibraryItem::from_entry(skill("b")),
        ])
    }

    fn profile_with(entries: Vec<Entry>) -> Profile {
        let mut profile = Profile::new("web");
        profile.entries = entries;
        profile
    }

    fn press(chooser: &mut Chooser, codes: &[KeyCode]) -> Choice {
        codes
            .iter()
            .fold(Choice::Open, |_, code| chooser.handle(key(*code)))
    }

    const SPACE: KeyCode = KeyCode::Char(' ');

    #[test]
    fn items_the_profile_has_start_checked_and_saving_keeps_them() {
        let mut chooser = Chooser::new(&profile_with(vec![skill("a")]), &library());
        chooser.outline.open_all();

        let choice = press(&mut chooser, &[KeyCode::Enter]);

        assert_eq!(choice, Choice::Done(vec![skill("a")]));
    }

    #[test]
    fn space_checks_and_unchecks_the_item_under_the_cursor() {
        let mut chooser = Chooser::new(&profile_with(vec![skill("a")]), &library());
        chooser.outline.open_all();

        // Section, a, b: check b, then uncheck a.
        press(&mut chooser, &[KeyCode::Down, KeyCode::Down, SPACE]);
        press(&mut chooser, &[KeyCode::Up, SPACE]);
        let choice = press(&mut chooser, &[KeyCode::Enter]);

        assert_eq!(choice, Choice::Done(vec![skill("b")]));
    }

    #[test]
    fn entries_the_library_does_not_offer_are_kept() {
        let mine = skill("mine");
        let mut chooser = Chooser::new(&profile_with(vec![mine.clone()]), &library());
        chooser.outline.open_all();

        press(&mut chooser, &[KeyCode::Down, SPACE]);
        let choice = press(&mut chooser, &[KeyCode::Enter]);

        assert_eq!(choice, Choice::Done(vec![mine, skill("a")]));
    }

    #[test]
    fn space_on_a_section_collapses_it_without_checking_anything() {
        let mut chooser = Chooser::new(&profile_with(vec![]), &library());
        chooser.outline.open_all();

        press(&mut chooser, &[SPACE]);

        assert_eq!(chooser.outline.visible().len(), 1);
        assert_eq!(press(&mut chooser, &[KeyCode::Enter]), Choice::Done(vec![]));
    }

    #[test]
    fn escape_closes_without_a_result() {
        let mut chooser = Chooser::new(&profile_with(vec![]), &library());
        chooser.outline.open_all();

        assert_eq!(press(&mut chooser, &[KeyCode::Esc]), Choice::Closed);
    }
}
