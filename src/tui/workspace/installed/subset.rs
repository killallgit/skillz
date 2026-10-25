//! The popup that picks which entries of the applied profile the project holds: what is
//! installed starts checked, and confirming installs the checked ones and removes the rest.

use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::apply::{holds, only_wanted_by};
use crate::profile::Profile;
use crate::tui::keymap::{self, Action, Scope};
use crate::tui::theme::Theme;
use crate::tui::widgets::list::ListView;
use crate::tui::widgets::modal::{self, ModalSize};
use crate::tui::widgets::panel::Panel;
use crate::workspace::Act;
use skillz_harness::Spec;
use skillz_harness::model::Installed;

const WIDTH: u16 = 70;
const MAX_HEIGHT: u16 = 24;

/// What a key did to the subset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Choice {
    Open,
    /// The checked entries are to be held and the unchecked ones are not.
    Done(Act),
    Closed,
}

#[derive(Debug, Clone)]
pub(super) struct Subset {
    profile: String,
    entries: Vec<(Spec, bool)>,
    list: ListView,
}

impl Subset {
    /// The profile's entries, those the project already holds checked.
    pub fn of(profile: &Profile, installed: &[Installed]) -> Self {
        let entries: Vec<(Spec, bool)> = profile
            .resolved()
            .into_iter()
            .map(|entry| {
                let held = holds(installed, &entry);
                (entry, held)
            })
            .collect();
        Subset {
            profile: profile.name.clone(),
            list: ListView::new(entries.len()),
            entries,
        }
    }

    pub fn handle(&mut self, key: KeyEvent, installed: &[Installed]) -> Choice {
        match keymap::resolve(Scope::Picker, key) {
            Some(Action::Select) => Choice::Done(self.act(installed)),
            Some(Action::Back) => Choice::Closed,
            Some(Action::Toggle) => {
                if let Some(at) = self.list.selected() {
                    self.entries[at].1 = !self.entries[at].1;
                }
                Choice::Open
            }
            Some(movement) => {
                self.list.apply(movement);
                Choice::Open
            }
            None => Choice::Open,
        }
    }

    fn act(&self, installed: &[Installed]) -> Act {
        let side = |checked: bool| -> Vec<Spec> {
            self.entries
                .iter()
                .filter(|(_, on)| *on == checked)
                .map(|(entry, _)| entry.clone())
                .collect()
        };
        let (kept, dropped) = (side(true), side(false));
        Act::Subset {
            remove: only_wanted_by(&dropped, &kept, installed),
            install: kept,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, theme: &Theme) {
        let size = ModalSize {
            width: WIDTH.min(frame.area().width),
            height: (self.entries.len() as u16 + 4)
                .min(MAX_HEIGHT)
                .min(frame.area().height),
        };
        let title = format!("Keep from {}", self.profile);
        let inner = modal::open(frame, size, Panel::new(title), theme);
        let [rows, _, hint] = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(inner);
        let glyphs = &theme.glyphs;
        let lines: Vec<Line<'static>> = self
            .entries
            .iter()
            .map(|(entry, on)| {
                let (mark, style) = match on {
                    true => (glyphs.checked, theme.ok),
                    false => (glyphs.unchecked, theme.muted),
                };
                Line::from(vec![
                    Span::styled(format!("{mark} "), style),
                    Span::raw(entry.to_string()),
                ])
            })
            .collect();
        self.list.render(frame, rows, &lines, theme);
        let help = Line::styled("space check  enter apply  esc cancel", theme.muted);
        frame.render_widget(Paragraph::new(help), hint);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::KeyCode;

    use super::*;
    use crate::tui::widgets::testing::key;
    use crate::tui::workspace::installed::rows::tests::installed;
    use skillz_harness::model::Kind;

    fn skill(name: &str) -> Spec {
        Spec::Skill {
            source: "o/r".into(),
            names: vec![name.into()],
            agents: vec![],
        }
    }

    fn profile() -> Profile {
        let mut profile = Profile::new("web");
        profile.entries = vec![skill("tdd"), skill("grill")];
        profile
    }

    fn have() -> Vec<Installed> {
        vec![installed("tdd", Kind::Skill, Some("o/r"))]
    }

    #[test]
    fn what_the_project_holds_starts_checked() {
        let subset = Subset::of(&profile(), &have());

        let checked: Vec<bool> = subset.entries.iter().map(|(_, on)| *on).collect();

        assert_eq!(checked, [true, false]);
    }

    #[test]
    fn unchecking_a_held_entry_removes_it_and_checking_a_missing_one_installs_it() {
        let have = have();
        let mut subset = Subset::of(&profile(), &have);

        subset.handle(key(KeyCode::Char(' ')), &have);
        subset.handle(key(KeyCode::Down), &have);
        subset.handle(key(KeyCode::Char(' ')), &have);
        let choice = subset.handle(key(KeyCode::Enter), &have);

        assert_eq!(
            choice,
            Choice::Done(Act::Subset {
                install: vec![skill("grill").with_agents(profile().agents)],
                remove: vec!["skill:tdd".into()],
            })
        );
    }

    #[test]
    fn escape_closes_without_an_act() {
        let have = have();
        let mut subset = Subset::of(&profile(), &have);

        assert_eq!(subset.handle(key(KeyCode::Esc), &have), Choice::Closed);
    }
}
