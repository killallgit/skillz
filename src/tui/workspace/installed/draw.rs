//! Drawing the Project and Global tabs: the header line, then the outline in a panel.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::header::{self, Comparison};
use super::rows::{Item, Row};
use super::status::Status;
use super::{InstalledTab, Popup};
use crate::app::apply::Drift;
use crate::tui::filter;
use crate::tui::theme::Theme;
use crate::tui::widgets::empty;
use crate::tui::widgets::modal::{self, ModalSize};
use crate::tui::widgets::panel::Panel;
use skillz_harness::model::Scope;

/// Room per level of depth before a row's caret or status mark.
pub(super) const INDENT: &str = "  ";
const SOURCE_WIDTH: u16 = 56;

impl InstalledTab {
    pub(super) fn draw_body(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let drift = self.drift();
        let comparison = self.comparison(&drift);
        let [head, body] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);
        let line = header::line(comparison, head.width, theme);
        frame.render_widget(Paragraph::new(line), head);
        self.draw_outline(frame, body, theme);
        match &mut self.popup {
            Popup::Apply(picker) => picker.render(frame, theme),
            Popup::Subset(subset) => subset.render(frame, theme),
            Popup::Save(field) => {
                let size = ModalSize {
                    width: SOURCE_WIDTH.min(frame.area().width),
                    height: 3,
                };
                let inner = modal::open(frame, size, Panel::new("Save as profile"), theme);
                field.render(frame, inner, theme);
            }
            Popup::Source(field) => {
                let size = ModalSize {
                    width: SOURCE_WIDTH.min(frame.area().width),
                    height: 3,
                };
                let inner = modal::open(frame, size, Panel::new("Add from repo"), theme);
                field.render(frame, inner, theme);
            }
            Popup::None | Popup::Filter => {}
        }
    }

    fn panel_title(&self) -> &str {
        match self.scope {
            Scope::Project => &self.dir,
            Scope::Global => "User-level, all projects",
        }
    }

    fn comparison<'a>(&self, drift: &'a Option<Drift>) -> Comparison<'a> {
        match drift {
            None => Comparison::Unchosen,
            Some(drift) => Comparison::With(drift),
        }
    }

    fn draw_outline(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let panel = Panel::new(filter::title(self.panel_title(), self.outline.query()))
            .focused(true)
            .bare()
            .badge(self.outline.badge());
        let inner = panel.render(frame, area, theme).inner(Margin::new(1, 0));
        if self.outline.visible().is_empty() {
            return empty::render(frame, inner, self.empty_message(theme));
        }
        let lines: Vec<Line<'static>> = self
            .outline
            .visible()
            .iter()
            .map(|&id| self.row_line(id, theme))
            .collect();
        self.outline.render(frame, inner, &lines, theme);
    }

    fn empty_message(&self, theme: &Theme) -> Vec<Line<'static>> {
        if self.outline.is_filtered() {
            return vec![Line::styled("Nothing matches", theme.muted)];
        }
        vec![
            Line::styled("Nothing installed here", theme.text),
            Line::styled("A adds a plugin or skills from a repo", theme.muted),
        ]
    }

    fn row_line(&self, id: usize, theme: &Theme) -> Line<'static> {
        let mut line = match &self.rows[id] {
            Row::Inherited => {
                let count = self.rows[id + 1..]
                    .iter()
                    .filter(|row| matches!(row, Row::Item(_)))
                    .count();
                let rule = theme.glyphs.rule.repeat(2);
                Line::from(format!(
                    "{rule} {} Global installs · read only ({count}) {rule}",
                    self.outline.caret(id, &theme.glyphs)
                ))
            }
            Row::Section(kind) => self.section_line(id, *kind, theme),
            Row::Package(group) => self.group_line(id, group, theme),
            Row::Item(item) => self.item_line(id, item, theme),
            Row::Detail(label) => Line::styled(
                format!("{}{label}", INDENT.repeat(self.outline.depth(id) + 2)),
                theme.muted,
            ),
        };
        if self.read_only(id) {
            let dim = theme.muted.add_modifier(Modifier::DIM);
            line.style = dim;
            for span in &mut line.spans {
                span.style = dim;
            }
        }
        line
    }

    /// The spinner, tick or cross of a change under way on the row; a space otherwise.
    fn status_mark(&self, id: &str, theme: &Theme) -> Span<'static> {
        let glyphs = &theme.glyphs;
        match self.statuses.of(id) {
            Some(Status::Working) => {
                Span::styled(glyphs.frame(self.statuses.ticks()), theme.accent)
            }
            Some(Status::Done) => Span::styled(glyphs.loaded, theme.ok),
            Some(Status::Failed) => Span::styled(glyphs.failed, theme.danger),
            None => Span::raw(" "),
        }
    }

    fn item_line(&self, row: usize, item: &Item, theme: &Theme) -> Line<'static> {
        let glyphs = &theme.glyphs;
        // Picks, change progress and update marks are keyed by id; a read-only row shows none
        // of them, whatever id a project row shares with it.
        let live = !self.read_only(row);
        let picked = live && self.pending.contains(&item.id);
        let (box_glyph, box_style, text_style) = match picked {
            false => (
                glyphs.checked,
                theme.ok,
                theme.text.add_modifier(Modifier::BOLD),
            ),
            true => (glyphs.to_remove, theme.warn, theme.warn),
        };
        let mut spans = vec![Span::raw(INDENT.repeat(self.outline.depth(row)))];
        if !item.children.is_empty() {
            spans.push(Span::styled(
                format!("{} ", self.outline.caret(row, glyphs)),
                theme.muted,
            ));
        }
        let status = match live {
            true => self.status_mark(&item.id, theme),
            false => Span::raw(" "),
        };
        spans.extend([status, Span::styled(format!(" {box_glyph} "), box_style)]);
        let outdated = live && self.is_outdated(&item.id);
        let text_style = match outdated {
            true => text_style.add_modifier(Modifier::BOLD),
            false => text_style,
        };
        spans.extend(filter::highlight(
            &item.label,
            &self.outline.matched(row),
            0,
            text_style,
            theme,
        ));
        if outdated {
            spans.push(Span::styled("*", text_style));
        }
        if !item.agents.is_empty() {
            spans.push(Span::styled(format!("  ({})", item.agents), theme.muted));
        }
        if !item.detail.is_empty() {
            spans.push(Span::styled(format!("  {}", item.detail), theme.muted));
        }
        if picked {
            spans.push(Span::styled("  to remove", text_style));
        }
        Line::from(spans)
    }
}
