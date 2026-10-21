//! What every filterable pane shares: the `/` prompt line, the title chip and the badge that
//! counts matches, and match highlighting.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::Paragraph;

use super::theme::Theme;
use super::widgets::input::InputField;

/// The `/` prompt with its text field, drawn where the key bar usually is.
pub fn draw_prompt(frame: &mut Frame, area: Rect, field: &InputField, theme: &Theme) {
    let [slash, text] = Layout::horizontal([Constraint::Length(1), Constraint::Min(1)]).areas(area);
    frame.render_widget(Paragraph::new("/").style(theme.accent), slash);
    field.render(frame, text, theme);
}

/// `title` with the active query appended; just the title when nothing is filtered.
pub fn title(title: &str, query: &str) -> String {
    match query {
        "" => title.to_string(),
        query => format!("{title} /{query}"),
    }
}

/// `n/shown` for a cursor on row `n`; a filtered pane also says how many rows it hides.
pub fn badge(selected: Option<usize>, shown: usize, total: usize, filtered: bool) -> String {
    let at = selected.map_or(0, |s| s + 1);
    match filtered {
        true => format!("{at}/{shown} of {total}"),
        false => format!("{at}/{shown}"),
    }
}

fn hit_style(theme: &Theme) -> Style {
    theme.accent.add_modifier(Modifier::UNDERLINED)
}

/// `text` as spans in `base`, with the characters at `positions` (shifted by `offset`) drawn
/// as matches.
pub fn highlight(
    text: &str,
    positions: &[usize],
    offset: usize,
    base: Style,
    theme: &Theme,
) -> Vec<Span<'static>> {
    let hit = |at: usize| at >= offset && positions.contains(&(at - offset));
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut run = String::new();
    let mut run_hit = false;
    for (at, c) in text.chars().enumerate() {
        let is_hit = hit(at);
        if is_hit != run_hit && !run.is_empty() {
            spans.push(span(std::mem::take(&mut run), run_hit, base, theme));
        }
        run_hit = is_hit;
        run.push(c);
    }
    if !run.is_empty() {
        spans.push(span(run, run_hit, base, theme));
    }
    spans
}

fn span(text: String, hit: bool, base: Style, theme: &Theme) -> Span<'static> {
    let style = if hit {
        base.patch(hit_style(theme))
    } else {
        base
    };
    Span::styled(text, style)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn underlined(span: &Span) -> bool {
        span.style.add_modifier.contains(Modifier::UNDERLINED)
    }

    #[test]
    fn matched_characters_are_underlined_and_the_text_is_unchanged() {
        let theme = Theme::dark();

        let spans = highlight("abcde", &[1, 2], 0, theme.text, &theme);

        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        let hits: Vec<bool> = spans.iter().map(underlined).collect();
        assert_eq!(text, "abcde");
        // Plain, matched, plain.
        assert_eq!(hits, [false, true, false]);
    }

    #[test]
    fn the_offset_shifts_positions_past_a_prefix() {
        let theme = Theme::dark();

        let spans = highlight("  abc", &[0], 2, theme.text, &theme);

        let hit: Vec<&str> = spans
            .iter()
            .filter(|s| underlined(s))
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(hit, ["a"]);
    }

    #[test]
    fn no_positions_is_one_plain_span() {
        let theme = Theme::dark();

        let spans = highlight("abc", &[], 0, theme.text, &theme);

        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].style, theme.text);
    }

    #[test]
    fn the_match_style_stays_distinct_without_colour() {
        let theme = Theme::mono();

        assert_ne!(hit_style(&theme), theme.text);
    }

    #[test]
    fn an_unfiltered_title_is_left_alone() {
        assert_eq!(title("Projects", ""), "Projects");
        assert_ne!(title("Projects", "x"), "Projects");
    }

    #[test]
    fn only_a_filtered_badge_mentions_the_total() {
        let plain = badge(Some(0), 3, 9, false);
        let filtered = badge(Some(0), 3, 9, true);

        assert!(!plain.contains('9'));
        assert!(filtered.contains('9'));
    }
}
