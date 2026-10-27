//! A text modal whose scroll position follows the wrapped content and current viewport.

use ratatui::Frame;
use ratatui::crossterm::event::{Event, MouseEventKind};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Wrap};

use super::container::{Container, Size};
use super::modal::{self, ModalSize, PAD_X};
use super::panel::Panel;
use crate::tui::keymap::Action;
use crate::tui::theme::Theme;

const WIDTH: u16 = 76;

/// What a text modal shows: the body, then the answers as buttons and a line of help text, each
/// only when given.
pub struct Page {
    pub panel: Panel,
    pub lines: Vec<Line<'static>>,
    pub buttons: Option<Line<'static>>,
    pub help: Option<Line<'static>>,
    /// A line (of the body, before wrapping) to scroll into view, when it is not already.
    pub reveal: Option<usize>,
}

#[derive(Default)]
pub struct TextModal {
    offset: usize,
    lines: usize,
    page: usize,
}

impl TextModal {
    /// Scroll the least that brings `line` into the window.
    fn reveal(&mut self, line: usize) {
        let window = self.page.max(1);
        if line < self.offset {
            self.offset = line;
        } else if line >= self.offset + window {
            self.offset = line + 1 - window;
        }
    }

    pub fn reset(&mut self) {
        self.offset = 0;
    }

    pub fn navigate(&mut self, action: Action) {
        let page = self.page.max(1);
        let half = (page / 2).max(1);
        self.offset = match action {
            Action::Up => self.offset.saturating_sub(1),
            Action::Down => self.offset.saturating_add(1),
            Action::PageUp => self.offset.saturating_sub(page),
            Action::PageDown => self.offset.saturating_add(page),
            Action::HalfPageUp => self.offset.saturating_sub(half),
            Action::HalfPageDown => self.offset.saturating_add(half),
            Action::Top => 0,
            Action::Bottom => self.lines,
            _ => self.offset,
        };
        self.clamp();
    }

    pub fn wheel(&mut self, event: &Event) {
        if let Event::Mouse(mouse) = event {
            match mouse.kind {
                MouseEventKind::ScrollUp => self.navigate(Action::Up),
                MouseEventKind::ScrollDown => self.navigate(Action::Down),
                _ => {}
            }
        }
    }

    pub fn render(&mut self, frame: &mut Frame, theme: &Theme, page: Page) {
        let before_reveal = page
            .reveal
            .map_or_else(Vec::new, |at| page.lines[..at].to_vec());
        let paragraph = Paragraph::new(page.lines).wrap(Wrap { trim: false });
        let width = WIDTH.min(frame.area().width.saturating_sub(2 * PAD_X));
        let lines = paragraph.line_count(width.saturating_sub(2));
        let (buttons, help) = (page.buttons.is_some(), page.help.is_some());
        let below = u16::from(buttons) + u16::from(help);
        // The body, then a blank line and the rows below it, and the border's two rows.
        let tail = if below > 0 { 1 + below } else { 0 };
        let size = ModalSize {
            width,
            height: u16::try_from(lines)
                .unwrap_or(u16::MAX)
                .saturating_add(tail + 2),
        };
        let inner = modal::open(frame, size, page.panel, theme);
        let [body, _, buttons_row, help_row] = Container::column([
            Size::Fill(1),
            Size::Fixed(u16::from(below > 0)),
            Size::Fixed(u16::from(buttons)),
            Size::Fixed(u16::from(help)),
        ])
        .areas(inner);
        self.lines = paragraph.line_count(body.width);
        self.page = usize::from(body.height);
        if page.reveal.is_some() {
            let row = Paragraph::new(before_reveal)
                .wrap(Wrap { trim: false })
                .line_count(body.width);
            self.reveal(row);
        }
        self.clamp();
        let offset = u16::try_from(self.offset).unwrap_or(u16::MAX);
        frame.render_widget(paragraph.scroll((offset, 0)), body);
        if let Some(line) = page.buttons {
            frame.render_widget(Paragraph::new(line).centered(), buttons_row);
        }
        if let Some(line) = page.help {
            frame.render_widget(Paragraph::new(line), help_row);
        }
    }

    fn clamp(&mut self) {
        self.offset = self.offset.min(self.lines.saturating_sub(self.page.max(1)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::render_to_buffer;

    fn page(lines: Vec<Line<'static>>, reveal: usize) -> Page {
        Page {
            panel: Panel::new("t"),
            lines,
            buttons: None,
            help: None,
            reveal: Some(reveal),
        }
    }

    #[test]
    fn revealing_a_line_below_wrapped_ones_scrolls_to_its_wrapped_row() {
        let long = "word ".repeat(60);
        let mut lines: Vec<Line<'static>> = (0..4).map(|_| Line::from(long.clone())).collect();
        lines.push(Line::from("target"));
        let at = lines.len() - 1;
        let mut modal = TextModal::default();

        render_to_buffer(40, 12, |frame| {
            modal.render(frame, &Theme::dark(), page(lines.clone(), at));
        });

        let row = modal.lines - 1;
        assert!(row >= at);
        assert!((modal.offset..modal.offset + modal.page).contains(&row));
    }
}
