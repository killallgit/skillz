//! Drawing the Library tab: the outline in a panel, and a line about the highlighted item.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::detail::{Detail, detail};
use super::rows::Row;
use super::{LibraryTab, Popup};
use crate::library::{Cue, Presence, Provenance};
use crate::tui::filter;
use crate::tui::theme::Theme;
use crate::tui::widgets::empty;
use crate::tui::widgets::modal::{self, ModalSize};
use crate::tui::widgets::panel::Panel;

const ITEM_INDENT: &str = "    ";
const ADD_WIDTH: u16 = 72;

impl LibraryTab {
    pub(super) fn draw_body(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let [body, foot] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);
        self.draw_outline(frame, body, theme);
        frame.render_widget(Paragraph::new(self.detail_line(theme)), foot);
        self.draw_popup(frame, theme);
    }

    fn draw_outline(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let panel = Panel::new(filter::title("Library", self.outline.query()))
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
            Line::styled("The library is empty", theme.text),
            Line::styled(
                "a adds an item, i syncs with what is installed",
                theme.muted,
            ),
        ]
    }

    fn row_line(&self, id: usize, theme: &Theme) -> Line<'static> {
        match &self.rows[id] {
            Row::Section { kind, count } => {
                let caret = self.outline.caret(id, &theme.glyphs);
                Line::from(vec![
                    Span::styled(format!("{caret} {}", kind.title()), theme.title),
                    Span::styled(format!("  {count}"), theme.muted),
                ])
            }
            Row::Item(item) => {
                let mut spans = vec![Span::styled(ITEM_INDENT, theme.text)];
                spans.extend(filter::highlight(
                    &item.label,
                    &self.outline.matched(id),
                    0,
                    theme.text,
                    theme,
                ));
                spans.push(Span::styled(presence_tags(self.presence[id]), theme.muted));
                if self.stale[id] {
                    spans.push(Span::styled(stale_tag(theme), theme.warn));
                }
                Line::from(spans)
            }
        }
    }

    fn detail_line(&self, theme: &Theme) -> Line<'static> {
        let Some(id) = self.outline.current() else {
            return Line::default();
        };
        match &self.rows[id] {
            Row::Item(item) => detail_text(&detail(item, self.stale[id]), theme),
            Row::Section { .. } => Line::default(),
        }
    }

    fn draw_popup(&mut self, frame: &mut Frame, theme: &Theme) {
        match &self.popup {
            Popup::Add(field) => {
                let size = ModalSize {
                    width: ADD_WIDTH.min(frame.area().width),
                    height: 3,
                };
                let inner = modal::open(frame, size, Panel::new("Add to library"), theme);
                field.render(frame, inner, theme);
            }
            Popup::Remove(dialog, _) => dialog.render(frame, theme),
            Popup::None | Popup::Filter => {}
        }
    }
}

fn detail_text(detail: &Detail, theme: &Theme) -> Line<'static> {
    let agents = match detail.agents.as_slice() {
        [] => "profile's agents".to_string(),
        some => some
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", "),
    };
    let provenance = detail.provenance.as_ref().map(provenance_text);
    let parts = [
        Some(cue_text(detail.cue).to_string()),
        Some(format!("{:?}", detail.kind)),
        Some(detail.source.clone()),
        Some(agents),
        provenance,
        detail
            .stale
            .then(|| format!("{} source missing", theme.glyphs.broken)),
    ];
    let line: Vec<String> = parts.into_iter().flatten().collect();
    Line::styled(format!(" {}", line.join(" · ")), theme.muted)
}

/// `link <url> · <engine> · <version>`, leaving out what was not recorded.
fn provenance_text(provenance: &Provenance) -> String {
    let link = provenance.link.as_ref().map(|link| format!("link {link}"));
    let version = provenance
        .version
        .as_ref()
        .map(|v| format!("version {}", v.chars().take(12).collect::<String>()));
    [link, Some(provenance.engine.to_string()), version]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ")
}

fn cue_text(cue: Cue) -> &'static str {
    match cue {
        Cue::Manual => "added by hand",
        Cue::Detected => "detected",
        Cue::Skillz => "added by skillz",
    }
}

/// `  ! missing source`, the tag of an item whose local folder is gone.
fn stale_tag(theme: &Theme) -> String {
    format!("  {} missing source", theme.glyphs.broken)
}

/// `  project  global` for where the item is installed now; nothing when it is nowhere.
fn presence_tags(presence: Presence) -> String {
    let tags = [(presence.project, "project"), (presence.global, "global")];
    tags.iter()
        .filter(|(here, _)| *here)
        .map(|(_, tag)| format!("  {tag}"))
        .collect()
}
