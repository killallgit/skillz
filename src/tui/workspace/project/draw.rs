//! Drawing the Project tab: the header line, then the outline in a panel.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::header;
use super::rows::{Item, Row, State, section_title};
use super::{Popup, ProjectTab};
use crate::tui::filter;
use crate::tui::theme::Theme;
use crate::tui::widgets::empty;
use crate::tui::widgets::panel::Panel;

/// Room before an item's label: the list's cursor gutter, its indent, the checkbox.
const ITEM_INDENT: &str = "  ";

impl ProjectTab {
    pub(super) fn draw_body(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let [head, body] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);
        let drift = self.drift();
        let line = header::line(&self.dir, drift.as_ref(), head.width, theme);
        frame.render_widget(Paragraph::new(line), head);
        self.draw_outline(frame, body, theme);
        if let Popup::Profiles(picker) = &mut self.popup {
            picker.render(frame, theme);
        }
    }

    fn draw_outline(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let panel = Panel::new(filter::title("Project", self.outline.query()))
            .focused(true)
            .badge(self.outline.badge());
        let inner = panel.render(frame, area, theme);
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
            Line::styled(
                "Add skills to the library (tab 2) to install them",
                theme.muted,
            ),
        ]
    }

    fn row_line(&self, id: usize, theme: &Theme) -> Line<'static> {
        match &self.rows[id] {
            Row::Section {
                kind,
                installed,
                total,
            } => {
                let caret = self.outline.caret(id, &theme.glyphs);
                Line::from(vec![
                    Span::styled(format!("{caret} {}", section_title(*kind)), theme.title),
                    Span::styled(format!("  {installed} of {total}"), theme.muted),
                ])
            }
            Row::Item(item) => self.item_line(id, item, theme),
        }
    }

    fn item_line(&self, id: usize, item: &Item, theme: &Theme) -> Line<'static> {
        let glyphs = &theme.glyphs;
        let (box_glyph, box_style, text_style) = match item.state {
            State::Installed(_) => (glyphs.checked, theme.ok, theme.text),
            State::Available(_) => (glyphs.unchecked, theme.muted, theme.muted),
        };
        let mut spans = vec![Span::styled(
            format!("{ITEM_INDENT}{box_glyph} "),
            box_style,
        )];
        spans.extend(filter::highlight(
            &item.label,
            &self.outline.matched(id),
            0,
            text_style,
            theme,
        ));
        if !item.detail.is_empty() {
            spans.push(Span::styled(format!("  {}", item.detail), theme.muted));
        }
        Line::from(spans)
    }
}
