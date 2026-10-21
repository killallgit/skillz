//! The line above the outline: the directory and how it compares with the chosen profile.

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::drift::Drift;
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

/// The pieces of the header, most important first.
pub(super) fn segments(dir: &str, drift: Option<&Drift>, theme: &Theme) -> Vec<Segment> {
    let mut shown = vec![seg(dir, theme.text)];
    match drift {
        None => shown.push(seg("no profile compared", theme.muted)),
        Some(d) if d.matches() => shown.push(seg(
            format!("{} matches {}", theme.glyphs.loaded, d.profile),
            theme.ok,
        )),
        Some(d) => {
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

pub(super) fn line(dir: &str, drift: Option<&Drift>, width: u16, theme: &Theme) -> Line<'static> {
    let mut spans = Vec::new();
    for (i, segment) in fit(segments(dir, drift, theme), width as usize)
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

    fn styles(drift: Option<&Drift>, theme: &Theme) -> Vec<Style> {
        segments("~/app", drift, theme)
            .iter()
            .map(|s| s.style)
            .collect()
    }

    #[test]
    fn a_match_is_ok_and_drift_names_what_is_missing_and_extra() {
        let theme = Theme::dark();

        let matching = styles(Some(&drift(0, 0)), &theme);
        let drifted = styles(Some(&drift(2, 1)), &theme);

        assert_eq!(matching, [theme.text, theme.ok]);
        assert_eq!(drifted, [theme.text, theme.danger, theme.warn, theme.muted]);
    }

    #[test]
    fn without_a_profile_the_badge_is_muted() {
        let theme = Theme::dark();

        assert_eq!(styles(None, &theme), [theme.text, theme.muted]);
    }

    #[test]
    fn segments_drop_from_the_end_and_the_directory_stays() {
        let theme = Theme::dark();
        let all = segments("~/app", Some(&drift(2, 1)), &theme);

        let wide = fit(all.clone(), 200).len();
        let narrow = fit(all.clone(), 16).len();
        let none = fit(all.clone(), 0).len();

        assert_eq!((wide, none), (all.len(), 1));
        assert!((1..all.len()).contains(&narrow));
    }
}
