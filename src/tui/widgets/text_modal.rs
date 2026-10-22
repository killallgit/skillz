//! A text modal whose scroll position follows the wrapped content and current viewport.

use ratatui::Frame;
use ratatui::crossterm::event::{Event, MouseEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Wrap};

use super::modal::{self, ModalSize};
use super::panel::Panel;
use crate::tui::keymap::Action;
use crate::tui::theme::Theme;

const WIDTH: u16 = 76;

pub struct Page {
    pub panel: Panel,
    pub lines: Vec<Line<'static>>,
    pub footer: Line<'static>,
}

#[derive(Default)]
pub struct TextModal {
    offset: usize,
    lines: usize,
    page: usize,
}

impl TextModal {
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
        let paragraph = Paragraph::new(page.lines).wrap(Wrap { trim: false });
        let width = WIDTH.min(frame.area().width);
        let lines = paragraph.line_count(width.saturating_sub(2));
        let size = ModalSize {
            width,
            height: u16::try_from(lines.saturating_add(3)).unwrap_or(u16::MAX),
        };
        let inner = modal::open(frame, size, page.panel, theme);
        let [body, foot] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
        self.lines = paragraph.line_count(body.width);
        self.page = usize::from(body.height);
        self.clamp();
        let offset = u16::try_from(self.offset).unwrap_or(u16::MAX);
        frame.render_widget(paragraph.scroll((offset, 0)), body);
        frame.render_widget(Paragraph::new(page.footer), foot);
    }

    fn clamp(&mut self) {
        self.offset = self.offset.min(self.lines.saturating_sub(self.page.max(1)));
    }
}
