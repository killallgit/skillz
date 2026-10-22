//! The popup that chooses the profile to apply to the project, and whether it replaces what is
//! there or only adds what is missing.

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::tui::keymap::{self, Action, Scope};
use crate::tui::theme::Theme;
use crate::tui::widgets::list::ListView;
use crate::tui::widgets::modal::{self, ModalSize};
use crate::tui::widgets::panel::Panel;

const WIDTH: u16 = 44;

/// What a key did to the picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Pick {
    Open,
    /// Apply `profile`; with `exact`, remove what it does not hold.
    Chosen {
        profile: String,
        exact: bool,
    },
    Closed,
}

#[derive(Debug, Clone)]
pub(super) struct Picker {
    names: Vec<String>,
    list: ListView,
    exact: bool,
}

impl Picker {
    /// The cursor starts on `current`, when it is one of `names`. Applying replaces by default.
    pub fn new(names: Vec<String>, current: Option<&str>) -> Self {
        let mut list = ListView::new(names.len());
        list.select(
            names
                .iter()
                .position(|n| Some(n.as_str()) == current)
                .unwrap_or(0),
        );
        Picker {
            names,
            list,
            exact: true,
        }
    }

    pub fn handle(&mut self, key: KeyEvent) -> Pick {
        if key.code == KeyCode::Char('m') {
            self.exact = !self.exact;
            return Pick::Open;
        }
        match keymap::resolve(Scope::Common, key) {
            Some(Action::Select) => self.chosen().map_or(Pick::Open, |profile| Pick::Chosen {
                profile,
                exact: self.exact,
            }),
            Some(Action::Back) => Pick::Closed,
            Some(movement) => {
                self.list.apply(movement);
                Pick::Open
            }
            None => Pick::Open,
        }
    }

    fn chosen(&self) -> Option<String> {
        self.names.get(self.list.selected()?).cloned()
    }

    pub fn render(&mut self, frame: &mut Frame, theme: &Theme) {
        let size = ModalSize {
            width: WIDTH.min(frame.area().width),
            height: (self.names.len() as u16 + 4).min(frame.area().height),
        };
        let inner = modal::open(frame, size, Panel::new("Apply profile"), theme);
        let [rows, _, mode] = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(inner);
        let lines: Vec<Line<'static>> = self.names.iter().map(|n| Line::from(n.clone())).collect();
        self.list.render(frame, rows, &lines, theme);
        let what = match self.exact {
            true => "replace everything",
            false => "add what is missing",
        };
        let line = Line::styled(format!("m  {what}"), theme.muted);
        frame.render_widget(Paragraph::new(line), mode);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::key;

    fn picker(current: Option<&str>) -> Picker {
        Picker::new(vec!["api".into(), "web".into()], current)
    }

    fn chosen(profile: &str, exact: bool) -> Pick {
        Pick::Chosen {
            profile: profile.into(),
            exact,
        }
    }

    #[test]
    fn enter_replaces_with_the_profile_under_the_cursor() {
        let mut picker = picker(None);

        picker.handle(key(KeyCode::Down));
        let pick = picker.handle(key(KeyCode::Enter));

        assert_eq!(pick, chosen("web", true));
    }

    #[test]
    fn m_switches_between_replacing_and_adding() {
        let mut picker = picker(None);

        picker.handle(key(KeyCode::Char('m')));
        let adding = picker.handle(key(KeyCode::Enter));
        picker.handle(key(KeyCode::Char('m')));
        let replacing = picker.handle(key(KeyCode::Enter));

        assert_eq!(
            (adding, replacing),
            (chosen("api", false), chosen("api", true))
        );
    }

    #[test]
    fn the_cursor_starts_on_the_applied_profile_and_escape_closes() {
        let mut picker = picker(Some("web"));

        let pick = picker.handle(key(KeyCode::Enter));
        let closed = picker.handle(key(KeyCode::Esc));

        assert_eq!(pick, chosen("web", true));
        assert_eq!(closed, Pick::Closed);
    }
}
