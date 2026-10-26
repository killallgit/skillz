//! A pane with a title and an optional right-aligned badge such as `3/12`: a rounded border for
//! dialogs and screens that must stand out, or `bare`, a single header line for the lists that
//! fill a tab.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::symbols::border::ROUNDED;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::tui::theme::Theme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Panel {
    title: String,
    badge: Option<String>,
    focused: bool,
    danger: bool,
    bare: bool,
}

impl Panel {
    pub fn new(title: impl Into<String>) -> Self {
        Panel {
            title: title.into(),
            badge: None,
            focused: false,
            danger: false,
            bare: false,
        }
    }

    pub fn badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Draw the border in the danger colour: what is inside can destroy something.
    pub fn danger(mut self, danger: bool) -> Self {
        self.danger = danger;
        self
    }

    /// No border: the title and badge sit on one line above the content.
    pub fn bare(mut self) -> Self {
        self.bare = true;
        self
    }

    pub fn block(&self, theme: &Theme) -> Block<'static> {
        let mut block = Block::new()
            .borders(Borders::ALL)
            .border_set(ROUNDED)
            .border_style(self.border_style(theme))
            .title(self.title_line(theme));
        if let Some(badge) = &self.badge {
            block = block.title(self.badge_line(badge, theme));
        }
        block
    }

    /// Draw the frame, or the header line of a bare pane, and return the area for the content.
    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) -> Rect {
        if self.bare {
            return self.render_header(frame, area, theme);
        }
        let block = self.block(theme);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        inner
    }

    fn render_header(&self, frame: &mut Frame, area: Rect, theme: &Theme) -> Rect {
        let [header, rest] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
        frame.render_widget(Paragraph::new(self.title_line(theme)), header);
        if let Some(badge) = &self.badge {
            frame.render_widget(Paragraph::new(self.badge_line(badge, theme)), header);
        }
        rest
    }

    fn border_style(&self, theme: &Theme) -> Style {
        if self.danger {
            theme.danger
        } else {
            theme.border_for(self.focused)
        }
    }

    fn title_line(&self, theme: &Theme) -> Line<'static> {
        let style = if self.focused {
            theme.title
        } else {
            theme.muted
        };
        Line::from(Span::styled(format!(" {} ", self.title), style))
    }

    fn badge_line(&self, badge: &str, theme: &Theme) -> Line<'static> {
        Line::from(Span::styled(format!(" {badge} "), theme.muted)).right_aligned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::render_to_buffer;

    fn draw(panel: &Panel, width: u16) -> ratatui::buffer::Buffer {
        let theme = Theme::dark();
        render_to_buffer(width, 3, |f| {
            panel.render(f, f.area(), &theme);
        })
    }

    #[test]
    fn corners_are_rounded() {
        let buffer = draw(&Panel::new("t"), 12);
        assert_eq!(buffer[(0, 0)].symbol(), "╭");
        assert_eq!(buffer[(11, 2)].symbol(), "╯");
    }

    #[test]
    fn focused_border_uses_the_focus_style() {
        let theme = Theme::dark();
        let buffer = draw(&Panel::new("t").focused(true), 12);
        assert_eq!(buffer[(0, 1)].fg, theme.focus_border.fg.unwrap());
    }

    #[test]
    fn unfocused_border_uses_the_plain_border_style() {
        let theme = Theme::dark();
        let buffer = draw(&Panel::new("t"), 12);
        assert_eq!(buffer[(0, 1)].fg, theme.border.fg.unwrap());
    }

    #[test]
    fn a_danger_border_wins_over_focus() {
        let theme = Theme::dark();
        let buffer = draw(&Panel::new("t").focused(true).danger(true), 12);
        assert_eq!(buffer[(0, 1)].fg, theme.danger.fg.unwrap());
    }

    #[test]
    fn badge_sits_against_the_right_corner() {
        let buffer = draw(&Panel::new("t").badge("3/12"), 20);
        assert_eq!(buffer[(14, 0)].symbol(), "3");
        assert_eq!(buffer[(15, 0)].symbol(), "/");
    }

    #[test]
    fn render_returns_the_interior() {
        let theme = Theme::dark();
        let mut inner = Rect::default();
        render_to_buffer(10, 5, |f| {
            inner = Panel::new("t").render(f, f.area(), &theme)
        });
        assert_eq!(inner, Rect::new(1, 1, 8, 3));
    }

    #[test]
    fn a_bare_pane_has_a_header_line_and_no_border() {
        let theme = Theme::dark();
        let mut inner = Rect::default();
        let buffer = render_to_buffer(20, 4, |f| {
            inner = Panel::new("t")
                .bare()
                .badge("3/12")
                .render(f, f.area(), &theme)
        });

        assert_eq!(inner, Rect::new(0, 1, 20, 3));
        let borders = ["╭", "╮", "╰", "╯", "│", "─"];
        assert!(
            buffer
                .content()
                .iter()
                .all(|cell| !borders.contains(&cell.symbol()))
        );
        assert_eq!(buffer[(1, 0)].symbol(), "t");
        assert_eq!(buffer[(18, 0)].symbol(), "2");
    }
}
