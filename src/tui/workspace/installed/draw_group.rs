//! Drawing the rows that stand for a package and for what a plugin provides.

use ratatui::text::{Line, Span};

use super::InstalledTab;
use super::draw::INDENT;
use super::rows::{Group, Owned, items_of};
use crate::tui::theme::Theme;

const CONTENTS_SEPARATOR: &str = ", ";

impl InstalledTab {
    pub(super) fn group_line(&self, id: usize, group: &Group, theme: &Theme) -> Line<'static> {
        let caret = self.outline.caret(id, &theme.glyphs);
        let indent = INDENT.repeat(self.outline.depth(id));
        let title = format!("{indent}{caret} {}", group.package.title());
        let contents: Vec<String> = group
            .contents
            .iter()
            .map(|(kind, count)| kind.counted(*count))
            .collect();
        let mut spans = vec![
            Span::styled(title, theme.title),
            Span::styled(
                format!("  {}", contents.join(CONTENTS_SEPARATOR)),
                theme.muted,
            ),
        ];
        spans.extend(self.pending_summary(id, theme));
        Line::from(spans)
    }

    pub(super) fn owned_line(&self, id: usize, owned: &Owned, theme: &Theme) -> Line<'static> {
        Line::from(vec![
            Span::raw(INDENT.repeat(self.outline.depth(id))),
            Span::styled(format!("  {}", owned.label), theme.text),
            Span::styled(format!("  {}", owned.kind.noun()), theme.muted),
            Span::styled(format!("  {}", owned.detail), theme.muted),
        ])
    }

    fn pending_summary(&self, group: usize, theme: &Theme) -> Option<Span<'static>> {
        let counts = self.pending.counts(items_of(&self.rows, group));
        let text = match (counts.install, counts.remove) {
            (0, 0) => return None,
            (install, 0) => format!("{install} to install"),
            (0, remove) => format!("{remove} to remove"),
            (install, remove) => format!("{install} to install, {remove} to remove"),
        };
        Some(Span::styled(format!("  {text}"), theme.accent))
    }
}
