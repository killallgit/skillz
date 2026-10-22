//! The popup that chooses a profile: the one the project is compared with, or the one to
//! replace the project with.

use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::text::Line;

use crate::tui::keymap::{self, Action, Scope};
use crate::tui::theme::Theme;
use crate::tui::widgets::list::ListView;
use crate::tui::widgets::modal::{self, ModalSize};
use crate::tui::widgets::panel::Panel;

const WIDTH: u16 = 36;
const NO_PROFILE: &str = "(no profile)";

/// What a key did to the picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Pick {
    Open,
    /// A profile, or `None` for no comparison.
    Chosen(Option<String>),
    Closed,
}

#[derive(Debug, Clone)]
pub(super) struct Picker {
    names: Vec<String>,
    list: ListView,
    title: &'static str,
    /// Whether the first row is "(no profile)".
    none_row: bool,
}

impl Picker {
    /// The cursor starts on `current`, when it is one of `names`.
    pub fn new(names: Vec<String>, current: Option<&str>) -> Self {
        let mut list = ListView::new(names.len() + 1);
        let at = names.iter().position(|n| Some(n.as_str()) == current);
        list.select(at.map_or(0, |at| at + 1));
        Picker {
            names,
            list,
            title: "Compare with",
            none_row: true,
        }
    }

    /// A list of just the profiles, to pick the one to apply.
    pub fn applying(names: Vec<String>) -> Self {
        Picker {
            list: ListView::new(names.len()),
            names,
            title: "Replace the project with",
            none_row: false,
        }
    }

    pub fn handle(&mut self, key: KeyEvent) -> Pick {
        match keymap::resolve(Scope::Common, key) {
            Some(Action::Select) => Pick::Chosen(self.chosen()),
            Some(Action::Back) => Pick::Closed,
            Some(movement) => {
                self.list.apply(movement);
                Pick::Open
            }
            None => Pick::Open,
        }
    }

    fn chosen(&self) -> Option<String> {
        let at = self
            .list
            .selected()?
            .checked_sub(usize::from(self.none_row))?;
        self.names.get(at).cloned()
    }

    pub fn render(&mut self, frame: &mut Frame, theme: &Theme) {
        let size = ModalSize {
            width: WIDTH,
            height: (self.names.len() as u16 + u16::from(self.none_row) + 2)
                .min(frame.area().height),
        };
        let inner = modal::open(frame, size, Panel::new(self.title), theme);
        let rows: Vec<Line<'static>> = self
            .none_row
            .then_some(NO_PROFILE)
            .into_iter()
            .chain(self.names.iter().map(String::as_str))
            .map(|name| Line::from(name.to_string()))
            .collect();
        self.list.render(frame, inner, &rows, theme);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::KeyCode;

    use super::*;
    use crate::tui::widgets::testing::key;

    fn picker(current: Option<&str>) -> Picker {
        Picker::new(vec!["api".into(), "web".into()], current)
    }

    #[test]
    fn enter_chooses_the_profile_under_the_cursor() {
        let mut picker = picker(None);

        picker.handle(key(KeyCode::Down));
        picker.handle(key(KeyCode::Down));
        let pick = picker.handle(key(KeyCode::Enter));

        assert_eq!(pick, Pick::Chosen(Some("web".into())));
    }

    #[test]
    fn the_first_row_chooses_no_profile() {
        let mut picker = picker(Some("web"));

        picker.handle(key(KeyCode::Home));
        let pick = picker.handle(key(KeyCode::Enter));

        assert_eq!(pick, Pick::Chosen(None));
    }

    #[test]
    fn the_cursor_starts_on_the_current_profile_and_escape_closes() {
        let mut picker = picker(Some("api"));

        let pick = picker.handle(key(KeyCode::Enter));
        let closed = picker.handle(key(KeyCode::Esc));

        assert_eq!(pick, Pick::Chosen(Some("api".into())));
        assert_eq!(closed, Pick::Closed);
    }
}
