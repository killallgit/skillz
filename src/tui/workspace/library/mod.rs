//! The Library tab: the catalog of skills, plugins, MCP servers, hooks and subagents that can be
//! installed or put in a profile. Still to come; the tab only holds its place.

use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::text::Line;

use super::act::Act;
use super::model::Model;
use super::tab::Tab;
use crate::tui::theme::Theme;
use crate::tui::widgets::empty;
use crate::tui::widgets::panel::Panel;

pub struct LibraryTab;

impl Tab for LibraryTab {
    fn title(&self) -> &'static str {
        "Library"
    }

    fn load(&mut self, _model: &Model) {}

    fn handle(&mut self, _key: KeyEvent) -> Option<Act> {
        None
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let inner = Panel::new("Library")
            .focused(true)
            .render(frame, area, theme);
        empty::render(
            frame,
            inner,
            vec![Line::styled("Nothing here yet", theme.muted)],
        );
    }
}
