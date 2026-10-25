//! Progress through a switch: a gauge over the steps and the log of those started, redrawn as
//! each step begins.

use std::cell::RefCell;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Gauge, Paragraph};

use super::shell::Ui;
use super::theme::Theme;
use super::widgets::modal::{self, ModalSize};
use super::widgets::panel::Panel;
use crate::app::prompt::Hold;
use crate::app::review::Progress;

const SIZE: ModalSize = ModalSize {
    width: 70,
    height: 14,
};

/// The steps seen so far; the last one is running.
struct State {
    title: String,
    total: usize,
    log: Vec<String>,
}

impl State {
    fn finished(&self) -> usize {
        self.log.len().saturating_sub(1)
    }

    fn ratio(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        (self.finished() as f64 / self.total as f64).min(1.0)
    }

    fn draw(&self, frame: &mut Frame, theme: &Theme) {
        let panel = Panel::new(self.title.clone()).focused(true);
        let inner = modal::open(frame, SIZE, panel, theme);
        let [bar, _, log] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .areas(inner);
        let label = format!("{}/{}", self.finished(), self.total);
        let gauge = Gauge::default()
            .ratio(self.ratio())
            .label(label)
            .gauge_style(theme.accent);
        frame.render_widget(gauge, bar);
        frame.render_widget(Paragraph::new(self.log_lines(log.height, theme)), log);
    }

    /// The newest `rows` log lines: finished ones marked done, the running one marked open.
    fn log_lines(&self, rows: u16, theme: &Theme) -> Vec<Line<'static>> {
        let skip = self.log.len().saturating_sub(usize::from(rows));
        let running = self.log.len().saturating_sub(1);
        self.log
            .iter()
            .enumerate()
            .skip(skip)
            .map(|(i, label)| {
                let (mark, style) = if i == running {
                    (theme.glyphs.open, theme.accent)
                } else {
                    (theme.glyphs.loaded, theme.ok)
                };
                Line::from(vec![
                    Span::styled(format!("{mark} "), style),
                    Span::raw(label.clone()),
                ])
            })
            .collect()
    }
}

/// Paints the progress screen on the shared terminal; keeps the terminal until dropped.
pub struct TuiProgress<'a> {
    ui: Ui<'a>,
    state: RefCell<State>,
    _hold: Hold<'a>,
}

impl<'a> TuiProgress<'a> {
    pub fn start(ui: Ui<'a>, title: &str, total: usize) -> Self {
        let progress = TuiProgress {
            ui,
            state: RefCell::new(State {
                title: title.to_string(),
                total,
                log: vec![],
            }),
            _hold: ui.shell.hold(),
        };
        progress.paint();
        progress
    }

    /// Progress is cosmetic: a terminal that fails to draw does not stop the work.
    fn paint(&self) {
        let state = self.state.borrow();
        let _ = self
            .ui
            .shell
            .paint(|frame| state.draw(frame, self.ui.theme));
    }
}

impl Progress for TuiProgress<'_> {
    fn step(&self, label: &str) {
        self.state.borrow_mut().log.push(label.to_string());
        self.paint();
    }
}

impl Drop for TuiProgress<'_> {
    fn drop(&mut self) {
        self.ui.shell.discard_input();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(log: usize, total: usize) -> State {
        State {
            title: "t".into(),
            total,
            log: (0..log).map(|i| i.to_string()).collect(),
        }
    }

    #[test]
    fn the_running_step_is_not_counted_as_finished() {
        let states = [state(0, 4), state(1, 4), state(3, 4)];

        let finished = states.map(|s| s.finished());

        // Nothing started, one running, and two behind the third.
        assert_eq!(finished, [0, 0, 2]);
    }

    #[test]
    fn the_ratio_never_passes_one() {
        let ratios = [
            state(2, 4).ratio(),
            state(9, 4).ratio(),
            state(3, 0).ratio(),
        ];

        // Assert one of four steps is finished, excess steps clamp, and an empty plan is zero.
        assert_eq!(ratios, [0.25, 1.0, 0.0]);
    }

    #[test]
    fn the_log_shows_the_newest_lines_with_the_running_one_last() {
        let theme = Theme::dark();

        let lines = state(5, 5).log_lines(2, &theme);

        // Two rows keep the last two steps; only the last has the running style.
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].spans[0].style, theme.ok);
        assert_eq!(lines[1].spans[0].style, theme.accent);
    }
}
