//! Drawing the Project and Global tabs: the header line, then the outline in a panel.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::header::{self, Comparison};
use super::rows::{self, Item, Owned, Part, Row, State};
use super::status::Status;
use super::{InstalledTab, Popup};
use crate::apply::Drift;
use crate::model::Scope;
use crate::tui::filter;
use crate::tui::theme::Theme;
use crate::tui::widgets::empty;
use crate::tui::widgets::panel::Panel;
use crate::tui::workspace::Tab;

/// Room per level of depth before a row's caret or status mark.
const INDENT: &str = "  ";
/// Cells of rule before and after the `Available` header's title.
const RULE_LEAD: usize = 2;
const RULE_TAIL: usize = 12;
const GLOBAL_TAG: &str = "global";

impl InstalledTab {
    pub(super) fn draw_body(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let [head, body] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);
        let drift = self.drift();
        let line = header::line(self.heading(), self.comparison(&drift), head.width, theme);
        frame.render_widget(Paragraph::new(line), head);
        self.draw_outline(frame, body, theme);
        match &mut self.popup {
            Popup::Profiles(picker) => picker.render(frame, theme),
            Popup::AlsoGlobal(dialog, _) => dialog.render(frame, theme),
            Popup::None | Popup::Filter => {}
        }
    }

    fn heading(&self) -> &str {
        match self.scope {
            Scope::Project => &self.dir,
            Scope::Global => "Global (user-level)",
        }
    }

    fn comparison<'a>(&self, drift: &'a Option<Drift>) -> Comparison<'a> {
        match (self.scope, drift) {
            (Scope::Global, _) => Comparison::Off,
            (Scope::Project, None) => Comparison::Unchosen,
            (Scope::Project, Some(drift)) => Comparison::With(drift),
        }
    }

    fn draw_outline(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let panel = Panel::new(filter::title(self.title(), self.outline.query()))
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
                "Add skills to the library (tab 3) to install them",
                theme.muted,
            ),
        ]
    }

    fn row_line(&self, id: usize, theme: &Theme) -> Line<'static> {
        match &self.rows[id] {
            Row::Available { total } => {
                let rule = |cells| theme.glyphs.rule.repeat(cells);
                let caret = self.outline.caret(id, &theme.glyphs);
                let title = format!("{caret} {} Available", rule(RULE_LEAD));
                Line::from(vec![
                    Span::styled(title, theme.title),
                    Span::styled(format!("  {total} {}", rule(RULE_TAIL)), theme.muted),
                ])
            }
            Row::Section { kind, total, part } => {
                let caret = self.outline.caret(id, &theme.glyphs);
                let indent = INDENT.repeat(self.outline.depth(id));
                let lead = match part {
                    Part::Installed => "",
                    Part::Available => "Available ",
                };
                let title = format!("{indent}{caret} {lead}{}", rows::title(*part, *kind));
                let mut spans = vec![
                    Span::styled(title, theme.title),
                    Span::styled(format!("  {total}"), theme.muted),
                ];
                spans.extend(self.pending_summary(id, theme));
                Line::from(spans)
            }
            Row::Item(item) => self.item_line(id, item, theme),
            Row::Owned(owned) => self.owned_line(id, owned, theme),
        }
    }

    fn owned_line(&self, id: usize, owned: &Owned, theme: &Theme) -> Line<'static> {
        Line::from(vec![
            Span::raw(INDENT.repeat(self.outline.depth(id))),
            Span::styled(format!("  {}", owned.label), theme.text),
            Span::styled(format!("  {}", owned.detail), theme.muted),
        ])
    }

    fn pending_summary(&self, section: usize, theme: &Theme) -> Option<Span<'static>> {
        let items = self.rows[section + 1..]
            .iter()
            .take_while(|row| matches!(row, Row::Item(_) | Row::Owned(_)))
            .filter_map(|row| match row {
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

    /// The spinner, tick or cross of a change under way on the row; a space otherwise.
    fn status_mark(&self, state: &State, theme: &Theme) -> Span<'static> {
        let glyphs = &theme.glyphs;
        match self.statuses.of(state) {
            Some(Status::Working) => {
                Span::styled(glyphs.frame(self.statuses.ticks()), theme.accent)
            }
            Some(Status::Done) => Span::styled(glyphs.loaded, theme.ok),
            Some(Status::Failed) => Span::styled(glyphs.failed, theme.danger),
            None => Span::raw(" "),
        }
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
        };
        let mut spans = vec![
            Span::raw(INDENT.repeat(self.outline.depth(id))),
            self.status_mark(&item.state, theme),
            Span::styled(format!(" {box_glyph} "), box_style),
        ];
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
        if item.global {
            spans.push(Span::styled(format!("  {GLOBAL_TAG}"), theme.accent));
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
