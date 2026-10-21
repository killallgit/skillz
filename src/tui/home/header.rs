//! The status line above the panes: version, current project and its profile, and counts.

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::model::Model;
use crate::tui::theme::Theme;

const SEPARATOR: &str = "  ";

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
pub(super) fn segments(model: &Model, theme: &Theme) -> Vec<Segment> {
    let glyphs = &theme.glyphs;
    let mut shown = vec![seg(
        format!("skillz v{}", env!("CARGO_PKG_VERSION")),
        theme.title,
    )];
    if let Some(current) = &model.current {
        shown.push(match &current.profile {
            Some(name) => seg(format!("{} {name}", glyphs.loaded), theme.ok),
            None => seg("no profile", theme.muted),
        });
        shown.push(seg(current.short.clone(), theme.text));
    }
    shown.push(seg(
        format!("{} projects", model.projects.len()),
        theme.muted,
    ));
    shown.push(seg(
        format!("{} profiles", model.profiles.len()),
        theme.muted,
    ));
    let missing = model.projects.iter().filter(|p| p.missing).count();
    if missing > 0 {
        shown.push(seg(
            format!("{} {missing} missing", glyphs.missing),
            theme.danger,
        ));
    }
    let broken = model.profiles.iter().filter(|p| p.broken).count();
    if broken > 0 {
        shown.push(seg(
            format!("{} {broken} broken", glyphs.broken),
            theme.danger,
        ));
    }
    shown
}

/// The segments that fit `width`, in order; the title always stays.
pub(super) fn fit(segments: Vec<Segment>, width: usize) -> Vec<Segment> {
    let mut used = 0;
    let mut kept = Vec::new();
    for (i, segment) in segments.into_iter().enumerate() {
        used += segment.text.width() + if i > 0 { SEPARATOR.width() } else { 0 };
        if i > 0 && used > width {
            break;
        }
        kept.push(segment);
    }
    kept
}

pub(super) fn line(model: &Model, width: u16, theme: &Theme) -> Line<'static> {
    let mut spans = Vec::new();
    for (i, segment) in fit(segments(model, theme), width as usize)
        .into_iter()
        .enumerate()
    {
        if i > 0 {
            spans.push(Span::raw(SEPARATOR));
        }
        spans.push(Span::styled(segment.text, segment.style));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::home::model::{Current, ProfileRow, ProjectRow};

    fn project(missing: bool) -> ProjectRow {
        ProjectRow {
            path: "p".into(),
            profile: None,
            short: "p".into(),
            missing,
            detail: vec![],
        }
    }

    fn model() -> Model {
        Model {
            current: Some(Current {
                short: "~/work/app".into(),
                profile: Some("web".into()),
            }),
            projects: vec![project(false), project(true)],
            profiles: vec![ProfileRow {
                name: "x".into(),
                detail: vec![],
                broken: true,
            }],
            notice: None,
        }
    }

    #[test]
    fn a_loaded_profile_badge_is_ok_and_the_problem_counts_are_danger() {
        let theme = Theme::dark();

        let shown = segments(&model(), &theme);

        let styles: Vec<Style> = shown.iter().map(|s| s.style).collect();
        // Title, badge, path, two counts, then missing and broken.
        assert_eq!(
            styles,
            [
                theme.title,
                theme.ok,
                theme.text,
                theme.muted,
                theme.muted,
                theme.danger,
                theme.danger
            ]
        );
    }

    #[test]
    fn no_problems_means_no_problem_segments() {
        let theme = Theme::dark();
        let mut healthy = model();
        healthy.projects = vec![project(false)];
        healthy.profiles.clear();

        let shown = segments(&healthy, &theme);

        assert!(shown.iter().all(|s| s.style != theme.danger));
    }

    #[test]
    fn an_unloaded_current_project_gets_a_muted_badge() {
        let theme = Theme::dark();
        let mut unloaded = model();
        unloaded.current.as_mut().unwrap().profile = None;

        let shown = segments(&unloaded, &theme);

        assert_eq!(shown[1].style, theme.muted);
    }

    #[test]
    fn segments_drop_from_the_end_and_the_title_stays() {
        let theme = Theme::dark();
        let all = segments(&model(), &theme);

        let wide = fit(all.clone(), 200).len();
        let narrow = fit(all.clone(), 30).len();
        let none = fit(all.clone(), 0).len();

        assert_eq!(wide, all.len());
        assert!((1..all.len()).contains(&narrow));
        assert_eq!(none, 1);
        assert_eq!(fit(all.clone(), 30)[0], all[0]);
    }
}
