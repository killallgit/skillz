//! Drawing the rows that stand for a section and for a package.

use ratatui::style::Modifier;
use ratatui::text::{Line, Span};

use super::InstalledTab;
use super::draw::INDENT;
use super::rows::{Group, items_of, plural};
use crate::app::library::ItemKind;
use crate::tui::theme::Theme;
use skillz_harness::model::Kind;

impl InstalledTab {
    pub(super) fn section_line(&self, id: usize, kind: Kind, theme: &Theme) -> Line<'static> {
        let caret = self.outline.caret(id, &theme.glyphs);
        let indent = INDENT.repeat(self.outline.depth(id));
        let count = items_of(&self.rows, id).count();
        let mut spans = vec![
            Span::styled(
                format!("{indent}{caret} {}", plural(kind)),
                ItemKind::of(kind)
                    .map_or(theme.title, |kind| theme.kind_style(kind))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" ({count})"), theme.muted),
        ];
        spans.extend(self.pending_summary(id, theme));
        Line::from(spans)
    }

    pub(super) fn group_line(&self, id: usize, group: &Group, theme: &Theme) -> Line<'static> {
        let caret = self.outline.caret(id, &theme.glyphs);
        let indent = INDENT.repeat(self.outline.depth(id));
        let mut spans = vec![
            Span::styled(
                format!("{indent}{caret} {}", group.package.title()),
                theme.title,
            ),
            Span::styled(
                format!(" ({})", items_of(&self.rows, id).count()),
                theme.muted,
            ),
        ];
        spans.extend(self.pending_summary(id, theme));
        Line::from(spans)
    }

    fn pending_summary(&self, row: usize, theme: &Theme) -> Option<Span<'static>> {
        match self.pending.count(items_of(&self.rows, row)) {
            0 => None,
            removing => Some(Span::styled(
                format!("  {removing} to remove"),
                theme.accent,
            )),
        }
    }
}
