//! A centred, cleared, bordered dialog area. The caller draws its content in the returned rect.

use ratatui::Frame;
use ratatui::layout::{Margin, Rect};
use ratatui::widgets::Clear;

use super::panel::Panel;
use crate::tui::theme::Theme;

/// Room between a dialog's border and what it holds, in columns and rows.
pub const PAD_X: u16 = 2;
pub const PAD_Y: u16 = 1;

/// Desired dialog size, border included and padding not; clamped to the screen when placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModalSize {
    pub width: u16,
    pub height: u16,
}

/// The rect of `size` centred in `screen`, never larger than `screen`.
pub fn centered(screen: Rect, size: ModalSize) -> Rect {
    let width = size.width.min(screen.width);
    let height = size.height.min(screen.height);
    Rect {
        x: screen.x + (screen.width - width) / 2,
        y: screen.y + (screen.height - height) / 2,
        width,
        height,
    }
}

/// Clear a centred area with room for the padding, draw the focused panel on it and return the
/// interior inside the padding.
pub fn open(frame: &mut Frame, size: ModalSize, panel: Panel, theme: &Theme) -> Rect {
    let padded = ModalSize {
        width: size.width.saturating_add(2 * PAD_X),
        height: size.height.saturating_add(2 * PAD_Y),
    };
    let area = centered(frame.area(), padded);
    frame.render_widget(Clear, area);
    panel
        .focused(true)
        .render(frame, area, theme)
        .inner(Margin::new(PAD_X, PAD_Y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::render_to_buffer;

    const SCREEN: Rect = Rect::new(0, 0, 80, 24);

    #[test]
    fn fitting_dialog_is_centred() {
        let area = centered(
            SCREEN,
            ModalSize {
                width: 40,
                height: 10,
            },
        );
        assert_eq!(area, Rect::new(20, 7, 40, 10));
    }

    #[test]
    fn oversized_dialog_shrinks_to_the_screen() {
        let area = centered(
            SCREEN,
            ModalSize {
                width: 200,
                height: 99,
            },
        );
        assert_eq!(area, SCREEN);
    }

    #[test]
    fn centring_respects_the_screen_origin() {
        let screen = Rect::new(10, 5, 20, 10);
        let area = centered(
            screen,
            ModalSize {
                width: 10,
                height: 4,
            },
        );
        assert_eq!(area, Rect::new(15, 8, 10, 4));
    }

    #[test]
    fn open_clears_what_was_underneath() {
        let theme = Theme::dark();
        let size = ModalSize {
            width: 10,
            height: 5,
        };
        let buffer = render_to_buffer(30, 9, |f| {
            f.buffer_mut()
                .set_string(15, 4, "#", ratatui::style::Style::new());
            open(f, size, Panel::new("t"), &theme);
        });
        assert_eq!(buffer[(15, 4)].symbol(), " ");
    }

    #[test]
    fn open_returns_the_interior_of_the_border() {
        let theme = Theme::dark();
        let size = ModalSize {
            width: 10,
            height: 5,
        };
        let mut inner = Rect::default();
        render_to_buffer(30, 9, |f| inner = open(f, size, Panel::new("t"), &theme));
        assert_eq!(inner, Rect::new(11, 3, 8, 3));
    }
}
