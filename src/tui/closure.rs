//! A screen built from a state and two closures, for the pickers whose logic lives in the state.

use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEventKind};

use super::app::{App, Screen, Step};
use super::shell::Ui;
use super::theme::Theme;

struct ClosureScreen<'a, S, D, T> {
    state: &'a mut S,
    draw: D,
    step: T,
}

impl<S, R, D, T> Screen<R> for ClosureScreen<'_, S, D, T>
where
    D: FnMut(&mut S, &mut Frame, &Theme),
    T: FnMut(&mut S, KeyEvent) -> Option<R>,
{
    fn handle(&mut self, event: &Event) -> Step<R> {
        let Some(key) = key_of(event) else {
            return Step::Stay;
        };
        match (self.step)(self.state, key) {
            Some(result) => Step::Act(result),
            None => Step::Stay,
        }
    }

    fn draw(&mut self, frame: &mut Frame, theme: &Theme) {
        (self.draw)(self.state, frame, theme);
    }
}

/// The key an event stands for: itself, or an arrow for the mouse wheel.
fn key_of(event: &Event) -> Option<KeyEvent> {
    let code = match event {
        Event::Key(key) => return Some(*key),
        Event::Mouse(mouse) => match mouse.kind {
            MouseEventKind::ScrollUp => KeyCode::Up,
            MouseEventKind::ScrollDown => KeyCode::Down,
            _ => return None,
        },
        _ => return None,
    };
    Some(KeyEvent::new(code, KeyModifiers::NONE))
}

/// Draw `state`, feed it keys until `step` yields a result; `None` when the session ends first.
pub fn event_loop<S, R>(
    ui: &Ui,
    state: &mut S,
    draw: impl FnMut(&mut S, &mut Frame, &Theme),
    step: impl FnMut(&mut S, KeyEvent) -> Option<R>,
) -> anyhow::Result<Option<R>> {
    ui.run(&mut App::new(ClosureScreen { state, draw, step }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::key;
    use ratatui::crossterm::event::{KeyModifiers as M, MouseEvent};

    fn wheel(kind: MouseEventKind) -> Event {
        Event::Mouse(MouseEvent {
            kind,
            column: 0,
            row: 0,
            modifiers: M::NONE,
        })
    }

    #[test]
    fn the_wheel_is_an_arrow_key_and_other_mouse_events_are_nothing() {
        let events = [
            wheel(MouseEventKind::ScrollUp),
            wheel(MouseEventKind::ScrollDown),
            wheel(MouseEventKind::Moved),
            Event::Key(key(KeyCode::Enter)),
        ];

        let codes: Vec<_> = events.iter().map(|e| key_of(e).map(|k| k.code)).collect();

        assert_eq!(
            codes,
            [
                Some(KeyCode::Up),
                Some(KeyCode::Down),
                None,
                Some(KeyCode::Enter)
            ]
        );
    }
}
