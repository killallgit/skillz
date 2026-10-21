//! The workspace's mouse: click to focus and select, double-click to apply, wheel to scroll.

use std::time::{Duration, Instant};

use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Position;

use super::model::{Act, Focus};
use super::state::{Home, Popup};
use crate::tui::keymap::Action;

const DOUBLE_CLICK: Duration = Duration::from_millis(400);
const WHEEL_ROWS: usize = 3;

/// The last click that landed on a row, to tell a double-click from two single ones.
#[derive(Debug, Clone, Copy)]
pub(super) struct Click {
    focus: Focus,
    row: usize,
    at: Instant,
}

impl Home {
    /// `now` is passed in so tests control the double-click window.
    pub fn on_mouse(&mut self, mouse: MouseEvent, now: Instant) -> Option<Act> {
        if !matches!(self.popup, Popup::None) {
            return None;
        }
        let at = Position::new(mouse.column, mouse.row);
        let focus = self.pane_at(at)?;
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => self.click(focus, at, now),
            MouseEventKind::ScrollUp => self.wheel(focus, Action::Up),
            MouseEventKind::ScrollDown => self.wheel(focus, Action::Down),
            _ => None,
        }
    }

    fn pane_at(&self, at: Position) -> Option<Focus> {
        [
            (Focus::Projects, &self.projects),
            (Focus::Profiles, &self.profiles),
        ]
        .into_iter()
        .find(|(_, pane)| pane.area.contains(at))
        .map(|(focus, _)| focus)
    }

    fn click(&mut self, focus: Focus, at: Position, now: Instant) -> Option<Act> {
        self.toasts.dismiss();
        self.focus_on(focus);
        let pane = self.pane_mut();
        let Some(shown) = pane.shown_at(at) else {
            self.last_click = None;
            return None;
        };
        pane.list.select(shown);
        let row = pane.row()?;
        let double = self
            .last_click
            .is_some_and(|c| c.focus == focus && c.row == row && now - c.at <= DOUBLE_CLICK);
        self.last_click = (!double).then_some(Click {
            focus,
            row,
            at: now,
        });
        if double { self.apply() } else { None }
    }

    fn wheel(&mut self, focus: Focus, movement: Action) -> Option<Act> {
        let pane = match focus {
            Focus::Projects => &mut self.projects,
            Focus::Profiles => &mut self.profiles,
        };
        for _ in 0..WHEEL_ROWS {
            pane.list.apply(movement);
        }
        None
    }
}
