//! Drawing the workspace: projects, profiles and detail panes, a key bar, toasts and popups.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Wrap};

use super::model::{Focus, ProfileRow, ProjectRow};
use super::state::{Home, Popup};
use crate::tui::help::{self, Hints};
use crate::tui::theme::{State, Theme};
use crate::tui::widgets::keybar::KeyBar;
use crate::tui::widgets::list::ListView;
use crate::tui::widgets::modal::{self, ModalSize};
use crate::tui::widgets::panel::Panel;

const HINTS: &Hints = &[
    ("tab", "pane"),
    ("enter", "apply"),
    ("u", "unload"),
    ("a", "add project"),
    ("n", "new profile"),
    ("s", "save as profile"),
    ("i", "add installed"),
    ("t", "add typed"),
    ("r", "remove entries"),
    ("d", "delete"),
    ("q", "quit"),
];

/// The width of the profile column in a project row (`{name:<14} `).
const PROFILE_COLUMN: usize = 15;

/// Room a row loses to the pane's borders, the cursor gutter and the scrollbar.
const ROW_MARGIN: usize = 5;

/// One list pane: its frame, cursor and rows.
struct Pane<'a> {
    title: &'static str,
    focused: bool,
    list: &'a mut ListView,
    rows: Vec<Line<'static>>,
}

impl Pane<'_> {
    fn render(self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let panel = Panel::new(self.title)
            .focused(self.focused)
            .position_badge(self.list.selected(), self.list.len());
        let inner = panel.render(frame, area, theme);
        self.list.render(frame, inner, &self.rows, theme);
    }
}

impl Home {
    pub fn draw(&mut self, frame: &mut Frame, theme: &Theme) {
        let [head, body, foot] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        frame.render_widget(Paragraph::new("skillz workspace").style(theme.title), head);
        self.draw_panes(frame, body, theme);
        KeyBar::new(HINTS).render(frame, foot, theme);
        self.toasts.render(frame, frame.area(), theme);
        self.draw_popup(frame, theme);
    }

    fn draw_panes(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let [projects, profiles, detail] = Layout::horizontal([
            Constraint::Percentage(40),
            Constraint::Percentage(25),
            Constraint::Percentage(35),
        ])
        .areas(area);
        let width = (projects.width as usize).saturating_sub(ROW_MARGIN);
        let rows = project_lines(&self.model.projects, width, theme);
        Pane {
            title: "Projects",
            focused: self.focus == Focus::Projects,
            list: &mut self.projects,
            rows,
        }
        .render(frame, projects, theme);
        let rows = profile_lines(&self.model.profiles, theme);
        Pane {
            title: "Profiles",
            focused: self.focus == Focus::Profiles,
            list: &mut self.profiles,
            rows,
        }
        .render(frame, profiles, theme);
        self.draw_detail(frame, detail, theme);
    }

    fn detail_lines(&self) -> &[String] {
        let detail = match self.focus {
            Focus::Projects => self
                .projects
                .selected()
                .map(|at| &self.model.projects[at].detail),
            Focus::Profiles => self
                .profiles
                .selected()
                .map(|at| &self.model.profiles[at].detail),
        };
        detail.map_or(&[], Vec::as_slice)
    }

    fn draw_detail(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let inner = Panel::new("Detail").render(frame, area, theme);
        let text = Paragraph::new(self.detail_lines().join("\n")).wrap(Wrap { trim: false });
        frame.render_widget(text, inner);
    }

    fn draw_popup(&self, frame: &mut Frame, theme: &Theme) {
        match &self.popup {
            Popup::None => {}
            Popup::Help => help::draw(frame, "Keys", HINTS, theme),
            Popup::Delete(_, dialog) => dialog.render(frame, theme),
            Popup::Input(ask, field) => {
                let size = ModalSize {
                    width: 64,
                    height: 5,
                };
                let inner = modal::open(frame, size, Panel::new(ask.title()), theme);
                field.render(frame, inner, theme);
            }
        }
    }
}

fn project_lines(projects: &[ProjectRow], width: usize, theme: &Theme) -> Vec<Line<'static>> {
    projects
        .iter()
        .map(|p| {
            let style = match p.missing {
                true => theme.state_style(State::Danger),
                false => theme.text,
            };
            Line::styled(fit_tail(&p.label, width, PROFILE_COLUMN), style)
        })
        .collect()
}

fn profile_lines(profiles: &[ProfileRow], theme: &Theme) -> Vec<Line<'static>> {
    profiles
        .iter()
        .map(|p| {
            let style = match p.broken {
                true => theme.state_style(State::Danger),
                false => theme.text,
            };
            Line::styled(p.name.clone(), style)
        })
        .collect()
}

/// `label` cut to `width`, keeping its first `head` characters and the end of the rest, since
/// the end of a path says the most.
pub(super) fn fit_tail(label: &str, width: usize, head: usize) -> String {
    let chars: Vec<char> = label.chars().collect();
    if chars.len() <= width || width <= head + 1 {
        return label.to_string();
    }
    let tail = width - head - 1;
    let start = chars.len() - tail;
    chars[..head]
        .iter()
        .chain(std::iter::once(&'…'))
        .chain(&chars[start..])
        .collect()
}
