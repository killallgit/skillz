//! The list rows of the projects and profiles panes: a state glyph, aligned columns, and the
//! characters the filter matched.

use ratatui::text::{Line, Span};

use super::model::{ProfileRow, ProjectRow};
use crate::matcher::match_positions;
use crate::tui::filter::highlight;
use crate::tui::theme::{State, Theme};
use crate::tui::widgets::table::{Column, Table, Width};

const PROFILE_COLUMN: u16 = 14;
/// A glyph and a space before each row's text.
const GLYPH_COLUMNS: usize = 2;
/// The cursor gutter and the scrollbar the list adds around a row.
const LIST_CHROME: usize = 3;

/// What a row needs to be drawn besides its own data.
pub(super) struct RowCtx<'a> {
    pub query: &'a str,
    /// The pane's inner width.
    pub width: u16,
    pub theme: &'a Theme,
}

pub(super) fn project_lines(rows: &[&ProjectRow], ctx: &RowCtx) -> Vec<Line<'static>> {
    let total = text_width(ctx.width);
    let table = Table::new(vec![
        Column::new("PROFILE", Width::Fixed(PROFILE_COLUMN)),
        Column::new("PROJECT", Width::Flex(1)),
    ]);
    let path_room = (total as usize).saturating_sub(PROFILE_COLUMN as usize + 1);
    rows.iter()
        .map(|p| {
            let cells = [
                p.profile.as_deref().unwrap_or("-"),
                &fit_tail(&p.short, path_room),
            ];
            let (glyph, state) = project_state(p, ctx.theme);
            row(glyph, state, table.row_text(&cells, total), ctx)
        })
        .collect()
}

pub(super) fn profile_lines(rows: &[&ProfileRow], ctx: &RowCtx) -> Vec<Line<'static>> {
    rows.iter()
        .map(|p| {
            let (glyph, state) = match p.broken {
                true => (ctx.theme.glyphs.broken, State::Danger),
                false => (" ", State::Ok),
            };
            row(glyph, state, p.name.clone(), ctx)
        })
        .collect()
}

fn project_state(project: &ProjectRow, theme: &Theme) -> (&'static str, State) {
    match (project.missing, project.loaded()) {
        (true, _) => (theme.glyphs.missing, State::Danger),
        (false, true) => (theme.glyphs.loaded, State::Ok),
        (false, false) => (" ", State::Muted),
    }
}

fn text_width(pane_width: u16) -> u16 {
    (pane_width as usize).saturating_sub(GLYPH_COLUMNS + LIST_CHROME) as u16
}

fn row(glyph: &str, state: State, text: String, ctx: &RowCtx) -> Line<'static> {
    let theme = ctx.theme;
    let text_style = match state {
        State::Danger => theme.danger,
        _ => theme.text,
    };
    let positions = match_positions(ctx.query, &text);
    let mut spans = vec![Span::styled(format!("{glyph} "), theme.state_style(state))];
    spans.extend(highlight(&text, &positions, 0, text_style, theme));
    Line::from(spans)
}

/// `label` cut to `width`, keeping the end of it, since the end of a path says the most.
pub(super) fn fit_tail(label: &str, width: usize) -> String {
    let chars: Vec<char> = label.chars().collect();
    if chars.len() <= width || width <= 1 {
        return label.to_string();
    }
    let tail = &chars[chars.len() - (width - 1)..];
    std::iter::once('…').chain(tail.iter().copied()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Modifier, Style};

    fn project(profile: Option<&str>, short: &str, missing: bool) -> ProjectRow {
        ProjectRow {
            path: short.into(),
            profile: profile.map(String::from),
            short: short.into(),
            missing,
            detail: vec![],
        }
    }

    fn ctx<'a>(query: &'a str, width: u16, theme: &'a Theme) -> RowCtx<'a> {
        RowCtx {
            query,
            width,
            theme,
        }
    }

    fn text_of(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn long_paths_keep_their_end_and_the_cell_fits_the_width() {
        let cut = fit_tail("~/Code/a/very/long/project/path", 12);

        assert_eq!(cut.chars().count(), 12);
        assert!(cut.ends_with("path") && cut.starts_with('…'));
        assert_eq!(fit_tail("short", 12), "short");
        assert_eq!(fit_tail("tiny", 1), "tiny", "too narrow to cut sensibly");
    }

    #[test]
    fn state_is_a_glyph_as_well_as_a_colour() {
        let theme = Theme::dark();
        let rows = [
            project(Some("web"), "~/a", false),
            project(None, "~/b", true),
            project(None, "~/c", false),
        ];
        let refs: Vec<&ProjectRow> = rows.iter().collect();

        let lines = project_lines(&refs, &ctx("", 60, &theme));

        let glyphs: Vec<&str> = lines.iter().map(|l| l.spans[0].content.trim()).collect();
        let styles: Vec<Style> = lines.iter().map(|l| l.spans[0].style).collect();
        assert_eq!(glyphs, [theme.glyphs.loaded, theme.glyphs.missing, ""]);
        assert_eq!(styles[..2], [theme.ok, theme.danger]);
    }

    #[test]
    fn project_rows_align_the_path_after_a_fixed_profile_column() {
        let theme = Theme::dark();
        let rows = [
            project(Some("web"), "~/a", false),
            project(None, "~/b", false),
        ];
        let refs: Vec<&ProjectRow> = rows.iter().collect();

        let lines = project_lines(&refs, &ctx("", 60, &theme));

        let starts: Vec<usize> = lines
            .iter()
            .map(|l| text_of(l).chars().position(|c| c == '~').unwrap())
            .collect();
        assert_eq!(starts[0], starts[1]);
    }

    #[test]
    fn the_matched_characters_are_underlined() {
        let theme = Theme::dark();
        let rows = [project(None, "~/alpha", false)];
        let refs: Vec<&ProjectRow> = rows.iter().collect();

        let lines = project_lines(&refs, &ctx("alp", 60, &theme));

        let hit: String = lines[0]
            .spans
            .iter()
            .filter(|s| s.style.add_modifier.contains(Modifier::UNDERLINED))
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(hit, "alp");
    }

    #[test]
    fn a_broken_profile_has_a_danger_glyph_and_a_healthy_one_is_blank() {
        let theme = Theme::dark();
        let rows = [
            ProfileRow {
                name: "bad".into(),
                detail: vec![],
                broken: true,
            },
            ProfileRow {
                name: "ok".into(),
                detail: vec![],
                broken: false,
            },
        ];
        let refs: Vec<&ProfileRow> = rows.iter().collect();

        let lines = profile_lines(&refs, &ctx("", 30, &theme));

        assert_eq!(lines[0].spans[0].style, theme.danger);
        assert_eq!(lines[1].spans[0].content, "  ");
    }
}
