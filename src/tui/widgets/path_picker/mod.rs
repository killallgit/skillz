//! A path field that lists the directories at the cursor as you type, instead of a file
//! explorer: edit the path, `Tab` steps into the highlighted directory.

mod suggest;

use std::path::{Path, PathBuf};

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::text::Line;

use super::input::{InputField, Outcome as Edit};
use super::list::ListView;
use super::modal::{self, ModalSize};
use super::panel::Panel;
use crate::tui::keymap::Action;
use crate::tui::theme::Theme;

const WIDTH: u16 = 64;
const LIST_ROWS: u16 = 8;
/// The border, the field, a gap and the hint.
const CHROME_ROWS: u16 = 5;

/// What a key did to the picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Open,
    Chosen(String),
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct PathPicker {
    title: String,
    field: InputField,
    home: PathBuf,
    names: Vec<String>,
    list: ListView,
}

impl PathPicker {
    /// A picker whose field starts at `start`, so deleting walks up from where skillz ran.
    pub fn new(title: impl Into<String>, start: &Path, home: &Path) -> Self {
        let typed = format!("{}/", start.display().to_string().trim_end_matches('/'));
        let mut picker = PathPicker {
            title: title.into(),
            field: InputField::new("path"),
            home: home.to_path_buf(),
            names: Vec::new(),
            list: ListView::new(0),
        };
        picker.field.replace(&typed);
        picker.refresh();
        picker
    }

    pub fn handle(&mut self, key: KeyEvent) -> Verdict {
        if let Some(movement) = movement(key) {
            self.list.apply(movement);
            return Verdict::Open;
        }
        if key.code == KeyCode::Tab {
            self.descend();
            return Verdict::Open;
        }
        match self.field.handle(key) {
            Edit::Cancel => Verdict::Cancelled,
            Edit::Submit if self.typed().is_empty() => Verdict::Open,
            Edit::Submit => Verdict::Chosen(self.typed().to_string()),
            Edit::Editing => {
                self.refresh();
                Verdict::Open
            }
        }
    }

    pub fn render(&mut self, frame: &mut Frame, theme: &Theme) {
        let rows = (self.names.len() as u16).clamp(1, LIST_ROWS);
        let size = ModalSize {
            width: WIDTH,
            height: CHROME_ROWS + rows,
        };
        let inner = modal::open(frame, size, Panel::new(self.title.clone()), theme);
        let [field, _, list, hint] = layout(inner);
        self.field.render(frame, field, theme);
        self.render_names(frame, list, theme);
        frame.render_widget(Line::styled(HINT, theme.muted), hint);
    }

    fn typed(&self) -> &str {
        self.field.value().trim()
    }

    fn descend(&mut self) {
        let Some(name) = self.list.selected().and_then(|i| self.names.get(i)) else {
            return;
        };
        let next = suggest::descend(self.field.value(), name);
        self.field.replace(&next);
        self.refresh();
    }

    fn refresh(&mut self) {
        self.names = suggest::complete(self.field.value(), &self.home);
        self.list = ListView::new(self.names.len());
    }

    fn render_names(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let rows: Vec<Line<'static>> = self
            .names
            .iter()
            .map(|name| Line::styled(format!("{name}/"), theme.text))
            .collect();
        self.list.render(frame, area, &rows, theme);
    }
}

const HINT: &str = "tab open folder   ↑↓ choose   enter add   esc cancel";

fn layout(inner: Rect) -> [Rect; 4] {
    let row = |y: u16, height: u16| Rect {
        y: inner.y + y,
        height: height.min(inner.height.saturating_sub(y)),
        ..inner
    };
    let list_height = inner.height.saturating_sub(3);
    [
        row(0, 1),
        row(1, 1),
        row(2, list_height),
        row(2 + list_height, 1),
    ]
}

fn movement(key: KeyEvent) -> Option<Action> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Up => Some(Action::Up),
        KeyCode::Down => Some(Action::Down),
        KeyCode::Char('p') if ctrl => Some(Action::Up),
        KeyCode::Char('n') if ctrl => Some(Action::Down),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::{ctrl, key, keys, render_to_buffer, row_text};
    use std::fs;

    fn project_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for name in ["api", "app", "docs"] {
            fs::create_dir(dir.path().join(name)).unwrap();
        }
        dir
    }

    fn picker(dir: &tempfile::TempDir) -> PathPicker {
        PathPicker::new("Add project", dir.path(), dir.path())
    }

    fn press(picker: &mut PathPicker, text: &str) {
        keys(text).into_iter().for_each(|k| {
            picker.handle(k);
        });
    }

    #[test]
    fn it_starts_at_the_directory_skillz_ran_in() {
        // Assemble
        let dir = project_dir();

        // Act
        let picker = picker(&dir);

        // Assert: the field holds the start path and lists its directories
        assert_eq!(picker.typed(), format!("{}/", dir.path().display()));
        assert_eq!(picker.names, ["api", "app", "docs"]);
    }

    #[test]
    fn deleting_back_to_a_parent_lists_its_directories() {
        let dir = project_dir();
        let mut picker = picker(&dir);

        press(&mut picker, "ap");
        picker.handle(key(KeyCode::Backspace));
        picker.handle(key(KeyCode::Backspace));

        assert_eq!(picker.names, ["api", "app", "docs"]);
    }

    #[test]
    fn typing_narrows_the_list() {
        let dir = project_dir();
        let mut picker = picker(&dir);

        press(&mut picker, "ap");

        assert_eq!(picker.names, ["api", "app"]);
    }

    #[test]
    fn tab_steps_into_the_highlighted_directory() {
        let dir = project_dir();
        let mut picker = picker(&dir);
        picker.handle(key(KeyCode::Down));

        picker.handle(key(KeyCode::Tab));

        assert_eq!(picker.typed(), format!("{}/app/", dir.path().display()));
    }

    #[test]
    fn enter_chooses_the_typed_path() {
        let dir = project_dir();
        let mut picker = picker(&dir);
        press(&mut picker, "docs");

        let verdict = picker.handle(key(KeyCode::Enter));

        assert_eq!(
            verdict,
            Verdict::Chosen(format!("{}/docs", dir.path().display()))
        );
    }

    #[test]
    fn escape_cancels() {
        let dir = project_dir();
        let mut picker = picker(&dir);

        assert_eq!(picker.handle(key(KeyCode::Esc)), Verdict::Cancelled);
    }

    #[test]
    fn control_keys_move_the_highlight_without_editing() {
        let dir = project_dir();
        let mut picker = picker(&dir);
        let before = picker.typed().to_string();

        picker.handle(ctrl('n'));

        assert_eq!(picker.list.selected(), Some(1));
        assert_eq!(picker.typed(), before);
    }

    #[test]
    fn it_draws_the_field_and_the_directories() {
        let dir = project_dir();
        let mut picker = picker(&dir);
        let theme = Theme::mono();

        let buffer = render_to_buffer(80, 24, |frame| picker.render(frame, &theme));

        let screen: String = (0..24).map(|y| row_text(&buffer, y)).collect();
        assert!(screen.contains("api/") && screen.contains("docs/"));
    }
}
