//! Drawing the Profiles tab: one list of profiles, each with its entries beneath it.

use ratatui::Frame;
use ratatui::layout::{Margin, Rect};
use ratatui::text::{Line, Span};

use super::rows::ListRow;
use super::{Popup, ProfilesTab};
use crate::tui::filter;
use crate::tui::theme::Theme;
use crate::tui::widgets::empty;
use crate::tui::widgets::panel::Panel;

const INDENT: &str = "  ";

impl ProfilesTab {
    pub(super) fn draw_body(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        self.draw_list(frame, area, theme);
        match &mut self.popup {
            Popup::Name(naming) => naming.render(frame, theme),
            Popup::Edit(editor) => editor.render(frame, theme),
            Popup::Delete(_, dialog) => dialog.render(frame, theme),
            Popup::None | Popup::Filter => {}
        }
    }

    fn draw_list(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let panel = Panel::new(filter::title("Profiles", self.list.query()))
            .focused(true)
            .badge(self.list.badge());
        let inner = panel.render(frame, area, theme).inner(Margin::new(1, 1));
        if self.list.visible().is_empty() {
            return empty::render(frame, inner, self.empty_list(theme));
        }
        let lines: Vec<Line<'static>> = (self.list.visible().iter())
            .map(|&id| self.row_line(id, theme))
            .collect();
        self.list.render(frame, inner, &lines, theme);
    }

    fn empty_list(&self, theme: &Theme) -> Vec<Line<'static>> {
        if self.list.is_filtered() {
            return vec![Line::styled("Nothing matches", theme.muted)];
        }
        vec![
            Line::styled("No profiles yet", theme.text),
            Line::styled("n makes one, s saves what the project has", theme.muted),
        ]
    }

    fn row_line(&self, id: usize, theme: &Theme) -> Line<'static> {
        match &self.list_rows[id] {
            ListRow::Profile(at) => self.profile_line(id, *at, theme),
            ListRow::Entry { label, .. } => Line::styled(
                format!("{}{label}", INDENT.repeat(self.list.depth(id) + 1)),
                theme.muted,
            ),
        }
    }

    fn profile_line(&self, id: usize, at: usize, theme: &Theme) -> Line<'static> {
        let row = &self.profiles[at];
        let caret = self.list.caret(id, &theme.glyphs);
        let mut spans = vec![Span::raw(format!("{caret} "))];
        spans.extend(filter::highlight(
            &row.profile.name,
            &self.list.matched(id),
            0,
            theme.text,
            theme,
        ));
        let count = row.profile.entries.len();
        let noun = if count == 1 { "entry" } else { "entries" };
        spans.push(Span::styled(format!("  {count} {noun}"), theme.muted));
        spans.push(match row.missing {
            0 => Span::styled("  matches the project", theme.ok),
            n => Span::styled(format!("  {n} missing from the project"), theme.warn),
        });
        Line::from(spans)
    }
}
