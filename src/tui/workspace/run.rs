//! The modal shown while a change runs: every item to change in a list, each with a spinner
//! while it works, a tick when it is done and a cross when it failed.

use ratatui::Frame;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::review::{ItemEvent, Phase};
use crate::tui::theme::Theme;
use crate::tui::widgets::modal::{self, ModalSize};
use crate::tui::widgets::panel::Panel;

const WIDTH: u16 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Waiting,
    Working,
    Done,
    Failed,
}

#[derive(Debug, Clone)]
pub(super) struct RunModal {
    title: String,
    items: Vec<(ItemEvent, State)>,
    ticks: usize,
}

impl RunModal {
    pub fn new(title: &str, planned: &[ItemEvent]) -> Self {
        RunModal {
            title: title.to_string(),
            items: planned
                .iter()
                .map(|e| (e.clone(), State::Waiting))
                .collect(),
            ticks: 0,
        }
    }

    pub fn tick(&mut self) {
        self.ticks += 1;
    }

    pub fn note(&mut self, event: &ItemEvent) {
        let state = match event.phase {
            Phase::Started => State::Working,
            Phase::Succeeded => State::Done,
            Phase::Failed(_) => State::Failed,
        };
        match self
            .items
            .iter_mut()
            .find(|(e, _)| e.target == event.target)
        {
            Some(known) => known.1 = state,
            None => self.items.push((event.clone(), state)),
        }
    }

    pub fn draw(&self, frame: &mut Frame, theme: &Theme) {
        let height = u16::try_from(self.items.len().max(1)).unwrap_or(u16::MAX);
        let screen = frame.area();
        let size = ModalSize {
            width: WIDTH.min(screen.width.saturating_sub(2 * modal::PAD_X)),
            height: height
                .saturating_add(2)
                .min(screen.height.saturating_sub(2 * modal::PAD_Y)),
        };
        let inner = modal::open(frame, size, Panel::new(self.title.clone()), theme);
        let rows = usize::from(inner.height);
        let skip = self.current().saturating_sub(rows.saturating_sub(1));
        let lines: Vec<Line<'static>> = self
            .items
            .iter()
            .skip(skip)
            .take(rows)
            .map(|(event, state)| self.line(event, *state, theme))
            .collect();
        frame.render_widget(Paragraph::new(lines), inner);
    }

    /// The row the work has reached: the first one still going, else the last.
    fn current(&self) -> usize {
        self.items
            .iter()
            .position(|(_, state)| matches!(state, State::Waiting | State::Working))
            .unwrap_or(self.items.len().saturating_sub(1))
    }

    fn line(&self, event: &ItemEvent, state: State, theme: &Theme) -> Line<'static> {
        let glyphs = &theme.glyphs;
        let (mark, style) = match state {
            State::Waiting => (glyphs.open, theme.muted),
            State::Working => (glyphs.frame(self.ticks), theme.accent),
            State::Done => (glyphs.loaded, theme.ok),
            State::Failed => (glyphs.failed, theme.danger),
        };
        let label = match state {
            State::Waiting => theme.muted,
            _ => theme.text,
        };
        Line::from(vec![
            Span::styled(format!("{mark} "), style),
            Span::styled(event.label.clone(), label),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::review::Target;

    fn planned(names: &[&str]) -> Vec<ItemEvent> {
        names
            .iter()
            .map(|n| ItemEvent::started(Target::Remove(n.to_string()), n.to_string()))
            .collect()
    }

    fn states(run: &RunModal) -> Vec<State> {
        run.items.iter().map(|(_, state)| *state).collect()
    }

    #[test]
    fn every_planned_item_waits_until_its_events_arrive() {
        let mut run = RunModal::new("Apply", &planned(&["a", "b"]));
        let a = planned(&["a"]).remove(0);

        run.note(&a);
        let working = states(&run);
        run.note(&a.ended(Ok(())));
        let done = states(&run);

        assert_eq!(working, [State::Working, State::Waiting]);
        assert_eq!(done, [State::Done, State::Waiting]);
    }

    #[test]
    fn an_item_nobody_planned_is_added_when_it_starts() {
        let mut run = RunModal::new("Apply", &[]);

        run.note(&planned(&["x"]).remove(0));

        assert_eq!(states(&run), [State::Working]);
    }

    #[test]
    fn the_list_follows_the_first_item_still_going() {
        let mut run = RunModal::new("Apply", &planned(&["a", "b", "c"]));
        let items = planned(&["a", "b"]);
        run.note(&items[0].ended(Ok(())));
        run.note(&items[1].ended(Err("x".into())));

        assert_eq!(run.current(), 2);
    }
}
