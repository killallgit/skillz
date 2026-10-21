//! Drawing the Project tab: the header line, then the outline in a panel.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::header;
use super::rows::{Item, Row, State};
use super::{Popup, ProjectTab};
use crate::tui::filter;
use crate::tui::theme::Theme;
use crate::tui::widgets::empty;
use crate::tui::widgets::panel::Panel;

/// Room before a section's caret, under its group's.
const SECTION_INDENT: &str = "  ";
/// Room before an item's label: the section's indent, then the checkbox.
const ITEM_INDENT: &str = "    ";

impl ProjectTab {
    pub(super) fn draw_body(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let [head, body] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);
        let drift = self.drift();
        let line = header::line(&self.dir, drift.as_ref(), head.width, theme);
        frame.render_widget(Paragraph::new(line), head);
        self.draw_outline(frame, body, theme);
        match &mut self.popup {
            Popup::Profiles(picker) => picker.render(frame, theme),
            Popup::AlsoGlobal(dialog, _) => dialog.render(frame, theme),
            Popup::None | Popup::Filter => {}
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
            Row::Group { group, total } => {
                let caret = self.outline.caret(id, &theme.glyphs);
                Line::from(vec![
                    Span::styled(format!("{caret} {}", group.title()), theme.title),
                    Span::styled(format!("  {total}"), theme.muted),
                ])
            }
            Row::Section { kind, total, .. } => {
                let caret = self.outline.caret(id, &theme.glyphs);
                let title = format!("{SECTION_INDENT}{caret} {}", kind.title());
                let mut spans = vec![
                    Span::styled(title, theme.title),
                    Span::styled(format!("  {total}"), theme.muted),
                ];
                spans.extend(self.pending_summary(id, theme));
                Line::from(spans)
            }
            Row::Item(item) => self.item_line(id, item, theme),
        }
    }

    fn pending_summary(&self, section: usize, theme: &Theme) -> Option<Span<'static>> {
        let items = self.rows[section + 1..].iter().map_while(|row| match row {
            Row::Item(item) => Some(item),
            _ => None,
        });
        let counts = self.pending.counts(items);
        let text = match (counts.install, counts.remove) {
            (0, 0) => return None,
            (install, 0) => format!("{install} to install"),
            (0, remove) => format!("{remove} to remove"),
            (install, remove) => format!("{install} to install, {remove} to remove"),
        };
        Some(Span::styled(format!("  {text}"), theme.accent))
    }

    fn item_line(&self, id: usize, item: &Item, theme: &Theme) -> Line<'static> {
        let glyphs = &theme.glyphs;
        let picked = self.pending.contains(&item.state);
        let (box_glyph, box_style, text_style, note) = match (&item.state, picked) {
            (State::Installed(_), false) => (glyphs.checked, theme.ok, theme.text, ""),
            (State::Available(_), false) => (glyphs.unchecked, theme.muted, theme.muted, ""),
            (State::Installed(_), true) => (glyphs.to_remove, theme.warn, theme.warn, "to remove"),
            (State::Available(_), true) => {
                (glyphs.to_install, theme.accent, theme.accent, "to install")
            }
            (State::Held { in_project: true }, _) => (glyphs.checked, theme.ok, theme.text, ""),
            (State::Held { in_project: false }, _) => {
                (glyphs.unchecked, theme.muted, theme.muted, "")
            }
        };
        let mut spans = vec![Span::styled(
            format!("{ITEM_INDENT}{box_glyph} "),
            box_style,
        )];
        let outdated = self.is_outdated(&item.state);
        let text_style = match outdated {
            true => text_style.add_modifier(Modifier::BOLD),
            false => text_style,
        };
        spans.extend(filter::highlight(
            &item.label,
            &self.outline.matched(id),
            0,
            text_style,
            theme,
        ));
        if outdated {
            spans.push(Span::styled("*", text_style));
        }
        if !item.detail.is_empty() {
            spans.push(Span::styled(format!("  {}", item.detail), theme.muted));
        }
        if picked {
            spans.push(Span::styled(format!("  {note}"), text_style));
        }
        Line::from(spans)
    }
}
