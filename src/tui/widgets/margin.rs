//! Breathing room between a full-screen layout and the terminal edge.

use ratatui::layout::{Margin, Rect};

/// Terminal cells are about twice as tall as wide, so the sides get twice the rows' margin.
pub const MARGIN_X: u16 = 2;
pub const MARGIN_Y: u16 = 1;

/// `screen` with the margin taken off every edge; empty when the screen is smaller than it.
pub fn inset(screen: Rect) -> Rect {
    screen.inner(Margin::new(MARGIN_X, MARGIN_Y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_edge_loses_the_margin() {
        let inner = inset(Rect::new(0, 0, 80, 24));
        assert_eq!(
            inner,
            Rect::new(MARGIN_X, MARGIN_Y, 80 - 2 * MARGIN_X, 24 - 2 * MARGIN_Y)
        );
    }

    #[test]
    fn a_screen_smaller_than_the_margin_is_empty() {
        assert!(inset(Rect::new(0, 0, 3, 1)).is_empty());
    }
}
