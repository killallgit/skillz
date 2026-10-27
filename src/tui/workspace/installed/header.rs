//! The line above the outline: where the items are and how they compare with the chosen profile.

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::app::apply::Drift;
use crate::tui::theme::Theme;

const GAP: &str = "  ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Segment {
    pub text: String,
    pub style: Style,
}

fn seg(text: impl Into<String>, style: Style) -> Segment {
    Segment {
        text: text.into(),
        style,
    }
}

/// How the items stand against the profile applied at the tab's level.
#[derive(Debug, Clone, Copy)]
pub(super) enum Comparison<'a> {
    Unchosen,
    With(&'a Drift),
}

/// The pieces of the header, most important first.
pub(super) fn segments(comparison: Comparison, theme: &Theme) -> Vec<Segment> {
    let mut shown = vec![];
    match comparison {
        Comparison::Unchosen => shown.push(seg("no profile applied", theme.muted)),
        Comparison::With(d) if d.matches() => shown.push(seg(
            format!("{} matches {}", theme.glyphs.loaded, d.profile),
            theme.ok,
        )),
        Comparison::With(d) => {
            if d.missing > 0 {
                shown.push(seg(format!("{} missing", d.missing), theme.danger));
            }
            if d.extra > 0 {
                shown.push(seg(format!("{} extra", d.extra), theme.warn));
            }
            shown.push(seg(format!("vs {}", d.profile), theme.muted));
        }
    }
    shown
}

/// The segments that fit `width`, in order; the first always stays.
fn fit(segments: Vec<Segment>, width: usize) -> Vec<Segment> {
    let mut used = 0;
    let mut kept = Vec::new();
    for (i, segment) in segments.into_iter().enumerate() {
        used += segment.text.width() + if i > 0 { GAP.width() } else { 0 };
        if i > 0 && used > width {
            break;
        }
        kept.push(segment);
    }
    kept
}

pub(super) fn line(comparison: Comparison, width: u16, theme: &Theme) -> Line<'static> {
    let mut spans = Vec::new();
    for (i, segment) in fit(segments(comparison, theme), width as usize)
        .into_iter()
        .enumerate()
    {
        if i > 0 {
            spans.push(Span::raw(GAP));
        }
        spans.push(Span::styled(segment.text, segment.style));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drift(missing: usize, extra: usize) -> Drift {
        Drift {
            profile: "web".into(),
            missing,
            extra,
        }
    }

    fn styles(comparison: Comparison, theme: &Theme) -> Vec<Style> {
        segments(comparison, theme)
            .iter()
            .map(|s| s.style)
            .collect()
    }

    #[test]
    fn a_match_is_ok_and_drift_names_what_is_missing_and_extra() {
        let theme = Theme::dark();

        let matching = styles(Comparison::With(&drift(0, 0)), &theme);
        let drifted = styles(Comparison::With(&drift(2, 1)), &theme);

        assert_eq!(matching, [theme.ok]);
        assert_eq!(drifted, [theme.danger, theme.warn, theme.muted]);
    }

    #[test]
    fn without_a_profile_the_badge_is_muted() {
        let theme = Theme::dark();

        assert_eq!(styles(Comparison::Unchosen, &theme), [theme.muted]);
    }

    #[test]
    fn segments_drop_from_the_end_and_the_directory_stays() {
        let theme = Theme::dark();
        let all = segments(Comparison::With(&drift(2, 1)), &theme);

        let wide = fit(all.clone(), 200).len();
        let narrow = fit(all.clone(), 16).len();
        let none = fit(all.clone(), 0).len();

        assert_eq!((wide, none), (all.len(), 1));
        assert!((1..all.len()).contains(&narrow));
    }
}
