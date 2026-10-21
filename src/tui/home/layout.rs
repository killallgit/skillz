//! How many panes fit: three side by side, two (lists stacked beside the detail), one behind
//! tabs, or none when the terminal is too small to use.

use ratatui::layout::{Constraint, Layout, Rect};

pub const MIN_WIDTH: u16 = 40;
pub const MIN_HEIGHT: u16 = 10;
const THREE_PANES: u16 = 100;
const TWO_PANES: u16 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Mode {
    #[default]
    Three,
    Two,
    Single,
    TooSmall,
}

/// Where each pane goes. In `Single` mode only the pane the tab names is drawn, in `projects`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Panes {
    pub projects: Rect,
    pub profiles: Rect,
    pub detail: Rect,
}

impl Mode {
    pub fn of(area: Rect) -> Self {
        match (area.width, area.height) {
            (w, h) if w < MIN_WIDTH || h < MIN_HEIGHT => Mode::TooSmall,
            (w, _) if w >= THREE_PANES => Mode::Three,
            (w, _) if w >= TWO_PANES => Mode::Two,
            _ => Mode::Single,
        }
    }

    pub fn panes(self, body: Rect) -> Panes {
        match self {
            Mode::Three => three(body),
            Mode::Two => two(body),
            Mode::Single | Mode::TooSmall => Panes {
                projects: body,
                profiles: body,
                detail: body,
            },
        }
    }
}

fn three(body: Rect) -> Panes {
    let [projects, profiles, detail] = Layout::horizontal([
        Constraint::Percentage(40),
        Constraint::Percentage(25),
        Constraint::Percentage(35),
    ])
    .areas(body);
    Panes {
        projects,
        profiles,
        detail,
    }
}

fn two(body: Rect) -> Panes {
    let [lists, detail] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(body);
    let [projects, profiles] =
        Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)]).areas(lists);
    Panes {
        projects,
        profiles,
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(width: u16, height: u16) -> Mode {
        Mode::of(Rect::new(0, 0, width, height))
    }

    #[test]
    fn width_picks_the_number_of_panes() {
        let modes = [200, 100, 99, 60, 59, 40].map(|w| mode(w, 30));

        assert_eq!(
            modes,
            [
                Mode::Three,
                Mode::Three,
                Mode::Two,
                Mode::Two,
                Mode::Single,
                Mode::Single
            ]
        );
    }

    #[test]
    fn too_narrow_or_too_short_is_too_small() {
        let small = [mode(39, 30), mode(120, 9), mode(0, 0)];

        assert_eq!(small, [Mode::TooSmall; 3]);
        assert_eq!(mode(40, 10), Mode::Single);
    }

    #[test]
    fn three_panes_sit_side_by_side_and_fill_the_body() {
        let body = Rect::new(0, 1, 120, 20);

        let panes = Mode::Three.panes(body);

        assert_eq!(
            panes.projects.width + panes.profiles.width + panes.detail.width,
            120
        );
        assert_eq!(panes.projects.height, 20);
    }

    #[test]
    fn two_panes_stack_the_lists_beside_the_detail() {
        let body = Rect::new(0, 1, 80, 20);

        let panes = Mode::Two.panes(body);

        assert_eq!(panes.projects.x, panes.profiles.x);
        assert_eq!(panes.profiles.y, panes.projects.bottom());
        assert_eq!(panes.detail.x, panes.projects.right());
        assert_eq!(panes.projects.height + panes.profiles.height, 20);
    }
}
