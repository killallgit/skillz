//! Drawing the workspace: header, panes for the terminal's size, a key bar, toasts and popups.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use super::header;
use super::layout::{MIN_HEIGHT, MIN_WIDTH, Mode};
use super::model::{DetailLine, Focus};
use super::pane::Pane;
use super::rows::{self, RowCtx};
use super::state::{Home, Popup};
use super::welcome;
use crate::tui::filter;
use crate::tui::keymap::{self, Scope};
use crate::tui::theme::Theme;
use crate::tui::widgets::empty;
use crate::tui::widgets::keybar::KeyBar;
use crate::tui::widgets::modal::{self, ModalSize};
use crate::tui::widgets::panel::Panel;

/// How one list pane is framed.
struct PaneFrame {
    title: &'static str,
    which: Focus,
    focused: bool,
}

/// Draw a filterable list pane; `make` turns the shown model rows into display lines.
fn draw_pane(
    frame: &mut Frame,
    area: Rect,
    spec: PaneFrame,
    pane: &mut Pane,
    theme: &Theme,
    make: impl FnOnce(&[usize], &RowCtx) -> Vec<Line<'static>>,
) {
    let badge = filter::badge(
        pane.list.selected(),
        pane.visible().len(),
        pane.total(),
        pane.is_filtered(),
    );
    let panel = Panel::new(filter::title(spec.title, pane.query()))
        .focused(spec.focused)
        .badge(badge);
    let inner = panel.render(frame, area, theme);
    pane.area = area;
    pane.rows = inner;
    if pane.visible().is_empty() {
        let hint = welcome::pane_hint(spec.which, pane.is_filtered(), theme);
        return empty::render(frame, inner, hint);
    }
    let ctx = RowCtx {
        query: pane.query(),
        width: inner.width,
        theme,
    };
    let lines = make(pane.visible(), &ctx);
    pane.list.render(frame, inner, &lines, theme);
}

impl Home {
    pub fn draw(&mut self, frame: &mut Frame, theme: &Theme) {
        let area = frame.area();
        self.mode = Mode::of(area);
        self.projects.area = Rect::default();
        self.profiles.area = Rect::default();
        self.detail.forget_area();
        if self.mode == Mode::TooSmall {
            return draw_too_small(frame, area, theme);
        }
        let [head, body, foot] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .areas(area);
        let line = header::line(&self.model, head.width, theme);
        frame.render_widget(Paragraph::new(line), head);
        self.draw_body(frame, body, theme);
        self.draw_footer(frame, foot, theme);
        self.toasts.render(frame, area, theme);
        self.draw_popup(frame, theme);
    }

    fn is_empty(&self) -> bool {
        self.model.projects.is_empty() && self.model.profiles.is_empty()
    }

    fn draw_body(&mut self, frame: &mut Frame, body: Rect, theme: &Theme) {
        if self.is_empty() {
            let inner = Panel::new("skillz")
                .focused(true)
                .render(frame, body, theme);
            return empty::render(frame, inner, welcome::welcome(theme));
        }
        if self.mode == Mode::Single {
            return self.draw_tabbed(frame, body, theme);
        }
        let panes = self.mode.panes(body);
        self.draw_projects(frame, panes.projects, theme);
        self.draw_profiles(frame, panes.profiles, theme);
        self.draw_detail(frame, panes.detail, theme);
    }

    fn draw_tabbed(&mut self, frame: &mut Frame, body: Rect, theme: &Theme) {
        let [tabs, pane] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(body);
        self.tabs.render(frame, tabs, theme);
        match self.tabs.selected() {
            0 => self.draw_projects(frame, pane, theme),
            1 => self.draw_profiles(frame, pane, theme),
            _ => self.draw_detail(frame, pane, theme),
        }
    }

    fn draw_projects(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let spec = PaneFrame {
            title: "Projects",
            which: Focus::Projects,
            focused: self.focus == Focus::Projects,
        };
        let model = &self.model.projects;
        draw_pane(
            frame,
            area,
            spec,
            &mut self.projects,
            theme,
            |shown, ctx| {
                let rows: Vec<_> = shown.iter().map(|&i| &model[i]).collect();
                rows::project_lines(&rows, ctx)
            },
        );
    }

    fn draw_profiles(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let spec = PaneFrame {
            title: "Profiles",
            which: Focus::Profiles,
            focused: self.focus == Focus::Profiles,
        };
        let model = &self.model.profiles;
        draw_pane(
            frame,
            area,
            spec,
            &mut self.profiles,
            theme,
            |shown, ctx| {
                let rows: Vec<_> = shown.iter().map(|&i| &model[i]).collect();
                rows::profile_lines(&rows, ctx)
            },
        );
    }

    /// The selected row and its detail lines.
    fn selected_detail(&self) -> (Option<(Focus, usize)>, &[DetailLine]) {
        let (row, detail) = match self.focus {
            Focus::Projects => (
                self.projects.row(),
                self.projects
                    .row()
                    .map(|at| &self.model.projects[at].detail),
            ),
            Focus::Profiles => (
                self.profiles.row(),
                self.profiles
                    .row()
                    .map(|at| &self.model.profiles[at].detail),
            ),
        };
        let at = row.map(|r| (self.focus, r));
        (at, detail.map_or(&[], Vec::as_slice))
    }

    fn draw_detail(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let (row, lines) = self.selected_detail();
        let lines = lines.to_vec();
        self.detail.draw(frame, area, row, (&lines, theme));
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if matches!(self.popup, Popup::Filter) {
            return filter::draw_prompt(frame, area, self.pane().field(), theme);
        }
        KeyBar::new(&keymap::hints(Scope::Home)).render(frame, area, theme);
    }

    fn draw_popup(&mut self, frame: &mut Frame, theme: &Theme) {
        match &mut self.popup {
            Popup::None | Popup::Filter => {}
            Popup::Help(help) => help.render(frame, theme),
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

fn draw_too_small(frame: &mut Frame, area: Rect, theme: &Theme) {
    let lines = vec![
        Line::styled("Terminal too small", theme.warn),
        Line::styled(format!("needs {MIN_WIDTH}x{MIN_HEIGHT}"), theme.muted),
    ];
    empty::render(frame, area, lines);
}
