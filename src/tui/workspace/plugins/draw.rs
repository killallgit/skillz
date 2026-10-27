//! Drawing the plugins list: a checkbox and a name per line, and the tools it is installed for.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};

use super::PluginsTab;
use super::rows::{DIVIDER, Plugin, Row};
use crate::app::merge::tag;
use crate::app::rule::centered_rule;
use crate::tui::filter;
use crate::tui::theme::Theme;
use crate::tui::widgets::buttons::Buttons;
use crate::tui::widgets::empty;
use crate::tui::widgets::panel::Panel;
use crate::tui::widgets::text_modal::Page;

/// Cells the list keeps before every line for its cursor.
const GUTTER: usize = 2;

impl PluginsTab {
    pub fn draw(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let inner = filter::pane(
            frame,
            area,
            self.outline.query(),
            self.outline.badge(),
            theme,
        );
        if self.outline.visible().is_empty() {
            return empty::render(frame, inner, self.empty_message(theme));
        }
        let lines: Vec<Line<'static>> = self
            .outline
            .visible()
            .iter()
            .map(|&id| self.row_line(id, inner.width, theme))
            .collect();
        self.outline.render(frame, inner, &lines, theme);
        self.draw_info(frame, theme);
    }

    fn draw_info(&mut self, frame: &mut Frame, theme: &Theme) {
        let Some(info) = &mut self.info else { return };
        let page = Page {
            panel: Panel::new(info.title.clone()),
            lines: info.lines.iter().cloned().map(Line::raw).collect(),
            buttons: Some(Buttons::new(&["OK"], 0).line(theme)),
            help: None,
            reveal: None,
        };
        info.modal.render(frame, theme, page);
    }

    fn empty_message(&self, theme: &Theme) -> Vec<Line<'static>> {
        if self.outline.is_filtered() {
            return vec![Line::styled("Nothing matches", theme.muted)];
        }
        vec![
            Line::styled("No plugins yet", theme.text),
            Line::styled("skillz install adds some", theme.muted),
        ]
    }

    fn row_line(&self, id: usize, width: u16, theme: &Theme) -> Line<'static> {
        match &self.rows[id] {
            Row::Own(plugin) => self.plugin_line(id, plugin, false, theme),
            Row::Global(plugin) => self.plugin_line(id, plugin, true, theme),
            Row::Gap => Line::default(),
            Row::Divider => divider_line(width, theme),
        }
    }

    /// A checkbox, the name and, while installed, the tools. A package that is only shown, not
    /// changeable, is all as dim as the tools of the others.
    fn plugin_line(
        &self,
        id: usize,
        plugin: &Plugin,
        shown_only: bool,
        theme: &Theme,
    ) -> Line<'static> {
        let glyphs = &theme.glyphs;
        // A flipped package shows the state it will have, in bold, until the change is applied.
        let flipped = self.pending.contains_key(&plugin.package);
        let wanted = plugin.is_installed() != flipped;
        let (mark, mark_style) = match (wanted, shown_only) {
            (true, false) => (glyphs.checked, theme.ok),
            (true, true) => (glyphs.checked, theme.muted),
            (false, _) => (glyphs.unchecked, theme.muted),
        };
        let mut name_style = if shown_only { theme.muted } else { theme.text };
        let mut mark_style = mark_style;
        if flipped {
            name_style = name_style.patch(theme.strong);
            mark_style = mark_style.patch(theme.strong);
        }
        let mut spans = vec![Span::styled(format!("{mark} "), mark_style)];
        spans.extend(filter::highlight(
            &plugin.name(),
            &self.outline.matched(id),
            0,
            name_style,
            theme,
        ));
        if plugin.is_installed() {
            spans.push(Span::styled(
                format!("  [{}]", tag(&plugin.tools)),
                theme.muted,
            ));
        }
        Line::from(spans)
    }
}

/// `──────── global ────────` across the list past its two-cell cursor gutter, the label in the
/// middle, as dim as a package's tools.
fn divider_line(width: u16, theme: &Theme) -> Line<'static> {
    let width = usize::from(width).saturating_sub(GUTTER);
    // The list keeps the styles of a line's spans, not of the line itself.
    Line::from(Span::styled(
        centered_rule(DIVIDER, width, theme.glyphs.rule),
        theme.muted,
    ))
}
