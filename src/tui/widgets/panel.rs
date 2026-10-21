//! A rounded, bordered pane with a title and an optional right-aligned badge such as `3/12`.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::symbols::border::ROUNDED;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders};

use crate::tui::theme::Theme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Panel {
    title: String,
    badge: Option<String>,
    focused: bool,
    danger: bool,
}

impl Panel {
    pub fn new(title: impl Into<String>) -> Self {
        Panel {
            title: title.into(),
            badge: None,
            focused: false,
            danger: false,
        }
    }

    pub fn badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    /// The `n/total` badge for a list position (one-based); empty lists show `0/0`.
    pub fn position_badge(self, selected: Option<usize>, total: usize) -> Self {
        self.badge(format!("{}/{total}", selected.map_or(0, |s| s + 1)))
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

    /// Draw the frame and return the area inside it.
    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) -> Rect {
        let block = self.block(theme);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        inner
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
    fn position_badge_is_one_based() {
        let panel = Panel::new("t").position_badge(Some(2), 12);
        assert_eq!(panel.badge.as_deref(), Some("3/12"));
    }

    #[test]
    fn empty_list_badge_is_zero_of_zero() {
        let panel = Panel::new("t").position_badge(None, 0);
        assert_eq!(panel.badge.as_deref(), Some("0/0"));
    }
}
