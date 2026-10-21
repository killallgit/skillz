//! What an empty workspace says: a welcome with three steps, and a hint per empty pane.

use ratatui::text::{Line, Span};

use super::model::Focus;
use crate::tui::keymap::{Action, Scope, keys_for};
use crate::tui::theme::Theme;

fn key(action: Action, theme: &Theme) -> Span<'static> {
    Span::styled(keys_for(Scope::Home, action), theme.accent)
}

fn text(words: &'static str, theme: &Theme) -> Span<'static> {
    Span::styled(words, theme.text)
}

/// The first-run screen: how to get from nothing to a profile applied.
pub(super) fn welcome(theme: &Theme) -> Vec<Line<'static>> {
    let step = |n: &'static str, action, words| {
        Line::from(vec![
            Span::styled(n, theme.muted),
            Span::raw("  "),
            key(action, theme),
            Span::raw("  "),
            text(words, theme),
        ])
    };
    vec![
        Line::styled("Welcome to skillz", theme.title),
        Line::raw(""),
        step("1", Action::AddProject, "add a project"),
        step("2", Action::SaveProfile, "save its setup as a profile"),
        step("3", Action::Select, "apply a profile to another project"),
    ]
}

/// The hint for a pane with no rows: a missing-data hint, or the way out of a filter.
pub(super) fn pane_hint(pane: Focus, filtered: bool, theme: &Theme) -> Vec<Line<'static>> {
    if filtered {
        return vec![
            Line::styled("Nothing matches", theme.muted),
            Line::from(vec![
                Span::styled("esc", theme.accent),
                text(" clears the filter", theme),
            ]),
        ];
    }
    match pane {
        Focus::Projects => vec![
            Line::styled("No projects yet", theme.muted),
            Line::from(vec![
                text("press ", theme),
                key(Action::AddProject, theme),
                text(" to add one", theme),
            ]),
        ],
        Focus::Profiles => vec![
            Line::styled("No profiles yet", theme.muted),
            Line::from(vec![
                text("press ", theme),
                key(Action::NewProfile, theme),
                text(" to create one, or ", theme),
                key(Action::SaveProfile, theme),
                text(" to save a project's setup", theme),
            ]),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accented_keys(lines: &[Line]) -> Vec<String> {
        let theme = Theme::dark();
        lines
            .iter()
            .flat_map(|l| &l.spans)
            .filter(|s| s.style == theme.accent)
            .map(|s| s.content.to_string())
            .collect()
    }

    #[test]
    fn the_welcome_names_the_keys_the_keymap_binds() {
        let theme = Theme::dark();

        let keys = accented_keys(&welcome(&theme));

        assert_eq!(
            keys,
            [Action::AddProject, Action::SaveProfile, Action::Select]
                .map(|a| keys_for(Scope::Home, a))
        );
    }

    #[test]
    fn each_empty_pane_points_at_a_key_that_fills_it() {
        let theme = Theme::dark();

        let projects = accented_keys(&pane_hint(Focus::Projects, false, &theme));
        let profiles = accented_keys(&pane_hint(Focus::Profiles, false, &theme));

        assert_eq!(projects, [keys_for(Scope::Home, Action::AddProject)]);
        assert_eq!(profiles.len(), 2);
    }

    #[test]
    fn a_filter_with_no_matches_says_so_whichever_pane_it_is() {
        let theme = Theme::dark();

        let a = pane_hint(Focus::Projects, true, &theme);
        let b = pane_hint(Focus::Profiles, true, &theme);

        assert_eq!(a, b);
    }
}
