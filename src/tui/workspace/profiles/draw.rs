//! Drawing the Profiles tab: the profile list beside the highlighted profile's entries.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};

use super::rows::{Row, section_title};
use super::{Focus, Popup, ProfilesTab};
use crate::tui::filter;
use crate::tui::theme::Theme;
use crate::tui::widgets::empty;
use crate::tui::widgets::panel::Panel;

impl ProfilesTab {
    pub(super) fn draw_body(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let [left, right] =
            Layout::horizontal([Constraint::Percentage(40), Constraint::Min(1)]).areas(area);
        self.draw_list(frame, left, theme);
        self.draw_entries(frame, right, theme);
        match &mut self.popup {
            Popup::Name(naming) => naming.render(frame, theme),
            Popup::Build(chooser) => chooser.render(frame, theme),
            Popup::Delete(_, dialog) => dialog.render(frame, theme),
            Popup::None | Popup::Filter => {}
        }
    }

    fn draw_list(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let panel = Panel::new(filter::title("Profiles", self.list.query()))
            .focused(self.focus == Focus::List)
            .badge(self.list.badge());
        let inner = panel.render(frame, area, theme);
        if self.list.visible().is_empty() {
            return empty::render(frame, inner, self.empty_list(theme));
        }
        let lines: Vec<Line<'static>> = (self.list.visible().iter())
            .map(|&id| self.profile_line(id, theme))
            .collect();
        self.list.render(frame, inner, &lines, theme);
    }

    fn empty_list(&self, theme: &Theme) -> Vec<Line<'static>> {
        if self.list.is_filtered() {
            return vec![Line::styled("Nothing matches", theme.muted)];
        }
        vec![
            Line::styled("No profiles yet", theme.text),
            Line::styled("n creates one, s saves what the project has", theme.muted),
        ]
    }

    fn profile_line(&self, id: usize, theme: &Theme) -> Line<'static> {
        let mut spans = filter::highlight(
            &self.profiles[id].profile.name,
            &self.list.matched(id),
            0,
            theme.text,
            theme,
        );
        spans.push(match self.profiles[id].missing {
            0 => Span::styled("  matches", theme.ok),
            n => Span::styled(format!("  {n} missing"), theme.warn),
        });
        Line::from(spans)
    }

    fn draw_entries(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let title = self.current().map_or("Entries", |p| p.name.as_str());
        let panel = Panel::new(title.to_string())
            .focused(self.focus == Focus::Entries)
            .badge(self.entries.badge());
        let inner = panel.render(frame, area, theme);
        if self.rows.is_empty() {
            let hint = "b builds it from the library";
            return empty::render(frame, inner, vec![Line::styled(hint, theme.muted)]);
        }
        let lines: Vec<Line<'static>> = (self.entries.visible().iter())
            .map(|&id| self.entry_line(id, theme))
            .collect();
        self.entries.render(frame, inner, &lines, theme);
    }

    fn entry_line(&self, id: usize, theme: &Theme) -> Line<'static> {
        match &self.rows[id] {
            Row::Section { kind, count } => {
                let caret = self.entries.caret(id, &theme.glyphs);
                Line::from(vec![
                    Span::styled(format!("{caret} {}", section_title(*kind)), theme.title),
                    Span::styled(format!(" ({count})"), theme.muted),
                ])
            }
            Row::Item(item) => Line::styled(format!("    {}", item.label), theme.text),
        }
    }
}
