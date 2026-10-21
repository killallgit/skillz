//! A single-line text field with a real cursor, readline-style editing.

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Position, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthChar;

use crate::tui::theme::Theme;

/// What a key did to the field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Editing,
    Submit,
    Cancel,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputField {
    value: String,
    /// Cursor position in characters, `0..=value.chars().count()`.
    cursor: usize,
    placeholder: String,
}

impl InputField {
    pub fn new(placeholder: impl Into<String>) -> Self {
        InputField {
            placeholder: placeholder.into(),
            ..Self::default()
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    #[cfg(test)]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Replace the text, leaving the cursor at its end.
    pub fn replace(&mut self, value: &str) {
        self.value = value.to_string();
        self.cursor = self.char_count();
    }

    pub fn handle(&mut self, key: KeyEvent) -> Outcome {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match (key.code, ctrl) {
            (KeyCode::Enter, _) => return Outcome::Submit,
            (KeyCode::Esc, _) => return Outcome::Cancel,
            (KeyCode::Char('u'), true) => self.delete_to_start(),
            (KeyCode::Char('w'), true) => self.delete_word_back(),
            (KeyCode::Char('a'), true) | (KeyCode::Home, _) => self.cursor = 0,
            (KeyCode::Char('e'), true) | (KeyCode::End, _) => self.cursor = self.char_count(),
            (KeyCode::Char(c), false) => self.insert(c),
            (code, _) => self.edit_with(code),
        }
        Outcome::Editing
    }

    fn edit_with(&mut self, code: KeyCode) {
        match code {
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(self.char_count()),
            KeyCode::Backspace => self.backspace(),
            KeyCode::Delete => self.delete_forward(),
            _ => {}
        }
    }

    fn char_count(&self) -> usize {
        self.value.chars().count()
    }

    fn byte_at(&self, chars: usize) -> usize {
        let end = self.value.len();
        self.value.char_indices().nth(chars).map_or(end, |(b, _)| b)
    }

    fn insert(&mut self, c: char) {
        self.value.insert(self.byte_at(self.cursor), c);
        self.cursor += 1;
    }

    fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.value.remove(self.byte_at(self.cursor));
        }
    }

    fn delete_forward(&mut self) {
        if self.cursor < self.char_count() {
            self.value.remove(self.byte_at(self.cursor));
        }
    }

    fn delete_to_start(&mut self) {
        self.value.drain(..self.byte_at(self.cursor));
        self.cursor = 0;
    }

    fn delete_word_back(&mut self) {
        let chars: Vec<char> = self.value.chars().collect();
        let mut start = self.cursor;
        while start > 0 && chars[start - 1].is_whitespace() {
            start -= 1;
        }
        while start > 0 && !chars[start - 1].is_whitespace() {
            start -= 1;
        }
        self.value
            .drain(self.byte_at(start)..self.byte_at(self.cursor));
        self.cursor = start;
    }

    /// First visible character and the cursor column, so the cursor stays inside `width`.
    fn window(&self, width: u16) -> (usize, u16) {
        let room = (width as usize).saturating_sub(1);
        let mut start = 0;
        let mut col = self.columns(0, self.cursor);
        while col > room && start < self.cursor {
            col -= self.columns(start, start + 1);
            start += 1;
        }
        (start, col as u16)
    }

    fn columns(&self, from: usize, to: usize) -> usize {
        let chars = self.value.chars().skip(from).take(to - from);
        chars.map(|c| c.width().unwrap_or(0)).sum()
    }

    /// Draw the field on the first row of `area`, and place the terminal
    /// cursor. Call only for the focused field.
    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let (start, col) = self.window(area.width);
        frame.render_widget(Paragraph::new(self.content_line(start, theme)), area);
        frame.set_cursor_position(Position::new(area.x + col, area.y));
    }

    fn content_line(&self, start: usize, theme: &Theme) -> Line<'static> {
        if self.value.is_empty() {
            return Line::from(Span::styled(self.placeholder.clone(), theme.muted));
        }
        let shown: String = self.value.chars().skip(start).collect();
        Line::from(Span::styled(shown, theme.text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::{ctrl, key, render_to_buffer};

    fn typed(text: &str) -> InputField {
        let mut field = InputField::new("p");
        for c in text.chars() {
            field.handle(key(KeyCode::Char(c)));
        }
        field
    }

    #[test]
    fn typing_appends_and_advances_the_cursor() {
        let field = typed("abc");
        assert_eq!((field.value(), field.cursor()), ("abc", 3));
    }

    #[test]
    fn insert_happens_at_the_cursor() {
        let mut field = typed("ac");
        field.handle(key(KeyCode::Left));
        field.handle(key(KeyCode::Char('b')));
        assert_eq!(field.value(), "abc");
    }

    #[test]
    fn backspace_removes_before_the_cursor() {
        let mut field = typed("abc");
        field.handle(key(KeyCode::Left));
        field.handle(key(KeyCode::Backspace));
        assert_eq!((field.value(), field.cursor()), ("ac", 1));
    }

    #[test]
    fn delete_removes_under_the_cursor() {
        let mut field = typed("abc");
        field.handle(key(KeyCode::Home));
        field.handle(key(KeyCode::Delete));
        assert_eq!(field.value(), "bc");
    }

    #[test]
    fn backspace_at_start_does_nothing() {
        let mut field = typed("a");
        field.handle(key(KeyCode::Home));
        field.handle(key(KeyCode::Backspace));
        assert_eq!(field.value(), "a");
    }

    #[test]
    fn home_and_end_jump_to_the_edges() {
        let mut field = typed("abc");
        field.handle(key(KeyCode::Home));
        assert_eq!(field.cursor(), 0);
        field.handle(key(KeyCode::End));
        assert_eq!(field.cursor(), 3);
    }

    #[test]
    fn ctrl_a_and_ctrl_e_match_home_and_end() {
        let mut field = typed("abc");
        field.handle(ctrl('a'));
        assert_eq!(field.cursor(), 0);
        field.handle(ctrl('e'));
        assert_eq!(field.cursor(), 3);
    }

    #[test]
    fn ctrl_u_deletes_everything_before_the_cursor() {
        let mut field = typed("abcd");
        field.handle(key(KeyCode::Left));
        field.handle(ctrl('u'));
        assert_eq!((field.value(), field.cursor()), ("d", 0));
    }

    #[test]
    fn ctrl_w_deletes_the_previous_word() {
        let mut field = typed("one two  ");
        field.handle(ctrl('w'));
        assert_eq!(field.value(), "one ");
    }

    #[test]
    fn multibyte_characters_edit_cleanly() {
        let mut field = typed("aéb");
        field.handle(key(KeyCode::Left));
        field.handle(key(KeyCode::Backspace));
        assert_eq!(field.value(), "ab");
    }

    #[test]
    fn enter_submits_and_escape_cancels() {
        let mut field = typed("a");
        assert_eq!(field.handle(key(KeyCode::Enter)), Outcome::Submit);
        assert_eq!(field.handle(key(KeyCode::Esc)), Outcome::Cancel);
    }

    #[test]
    fn empty_field_renders_the_placeholder_muted() {
        let theme = Theme::dark();
        let field = InputField::new("name");
        let buffer = render_to_buffer(10, 1, |f| field.render(f, f.area(), &theme));
        assert_eq!(buffer[(0, 0)].fg, theme.muted.fg.unwrap());
    }

    #[test]
    fn long_text_scrolls_so_the_cursor_stays_inside() {
        let field = typed("abcdefghij");
        let (start, col) = field.window(5);
        assert_eq!((start, col), (6, 4));
    }
}
