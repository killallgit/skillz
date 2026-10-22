//! Semantic colour roles. Screens ask for a role, never for a colour.

use ratatui::style::{Color, Modifier, Style};

use super::glyphs::Glyphs;
use crate::library::ItemKind;

/// How a status reads at a glance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Ok,
    Warn,
    Danger,
    Muted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub accent: Style,
    pub muted: Style,
    pub ok: Style,
    pub warn: Style,
    pub danger: Style,
    pub selection: Style,
    pub focus_border: Style,
    pub border: Style,
    pub title: Style,
    pub text: Style,
    pub kinds: Kinds,
    pub glyphs: Glyphs,
}

/// One style per kind of item, none of them the greens, yellows and reds that mean a state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kinds {
    skill: Style,
    plugin: Style,
    subagent: Style,
    mcp: Style,
    hook: Style,
}

impl Kinds {
    fn of(colours: [u8; 5]) -> Self {
        let [skill, plugin, subagent, mcp, hook] = colours.map(|c| fg(Color::Indexed(c)));
        Kinds {
            skill,
            plugin,
            subagent,
            mcp,
            hook,
        }
    }

    /// Kinds told apart by the word alone.
    fn plain() -> Self {
        Kinds {
            skill: Style::new(),
            plugin: Style::new(),
            subagent: Style::new(),
            mcp: Style::new(),
            hook: Style::new(),
        }
    }
}

fn fg(color: Color) -> Style {
    Style::new().fg(color)
}

fn modifier(m: Modifier) -> Style {
    Style::new().add_modifier(m)
}

impl Theme {
    pub fn dark() -> Self {
        Theme {
            accent: fg(Color::Cyan),
            muted: fg(Color::DarkGray),
            ok: fg(Color::Green),
            warn: fg(Color::Yellow),
            danger: fg(Color::Red),
            selection: Style::new().bg(Color::Indexed(238)),
            focus_border: fg(Color::Cyan),
            border: fg(Color::DarkGray),
            title: fg(Color::White).add_modifier(Modifier::BOLD),
            text: Style::new(),
            kinds: Kinds::of([75, 177, 80, 180, 146]),
            glyphs: Glyphs::UNICODE,
        }
    }

    pub fn light() -> Self {
        Theme {
            accent: fg(Color::Blue),
            muted: fg(Color::Gray),
            ok: fg(Color::Green),
            warn: fg(Color::Indexed(130)),
            danger: fg(Color::Red),
            selection: Style::new().bg(Color::Indexed(254)),
            focus_border: fg(Color::Blue),
            border: fg(Color::Gray),
            title: fg(Color::Black).add_modifier(Modifier::BOLD),
            text: Style::new(),
            kinds: Kinds::of([25, 127, 30, 94, 60]),
            glyphs: Glyphs::UNICODE,
        }
    }

    /// No colour at all: roles are told apart by modifiers only.
    pub fn mono() -> Self {
        Theme {
            accent: modifier(Modifier::BOLD),
            muted: modifier(Modifier::DIM),
            ok: Style::new(),
            warn: modifier(Modifier::UNDERLINED),
            danger: modifier(Modifier::BOLD | Modifier::UNDERLINED),
            selection: modifier(Modifier::REVERSED),
            focus_border: modifier(Modifier::BOLD),
            border: modifier(Modifier::DIM),
            title: modifier(Modifier::BOLD),
            text: Style::new(),
            kinds: Kinds::plain(),
            glyphs: Glyphs {
                cursor: "> ",
                ..Glyphs::UNICODE
            },
        }
    }

    /// Pick a theme from the environment, read through `lookup` so tests need no real env.
    pub fn detect(lookup: &dyn Fn(&str) -> Option<String>) -> Self {
        let base = Self::coloured(lookup);
        match Glyphs::detect(lookup) {
            Glyphs::ASCII => Theme {
                glyphs: Glyphs::ASCII,
                ..base
            },
            _ => base,
        }
    }

    fn coloured(lookup: &dyn Fn(&str) -> Option<String>) -> Self {
        if let Some(named) = lookup("SKILLZ_THEME").and_then(|n| Self::named(&n)) {
            return named;
        }
        if wants_no_colour(lookup) {
            return Self::mono();
        }
        match terminal_background(lookup) {
            Background::Light => Self::light(),
            Background::Dark => Self::dark(),
        }
    }

    fn named(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "dark" => Some(Self::dark()),
            "light" => Some(Self::light()),
            "mono" => Some(Self::mono()),
            _ => None,
        }
    }

    pub fn state_style(&self, state: State) -> Style {
        match state {
            State::Ok => self.ok,
            State::Warn => self.warn,
            State::Danger => self.danger,
            State::Muted => self.muted,
        }
    }

    pub fn kind_style(&self, kind: ItemKind) -> Style {
        match kind {
            ItemKind::Skill => self.kinds.skill,
            ItemKind::Plugin => self.kinds.plugin,
            ItemKind::Subagent => self.kinds.subagent,
            ItemKind::Mcp => self.kinds.mcp,
            ItemKind::Hook => self.kinds.hook,
        }
    }

    pub fn border_for(&self, focused: bool) -> Style {
        if focused {
            self.focus_border
        } else {
            self.border
        }
    }
}

enum Background {
    Light,
    Dark,
}

fn wants_no_colour(lookup: &dyn Fn(&str) -> Option<String>) -> bool {
    let no_color = lookup("NO_COLOR").is_some_and(|v| !v.is_empty());
    no_color || lookup("TERM").is_some_and(|t| t == "dumb")
}

/// `COLORFGBG` is `fg;bg` (sometimes `fg;default;bg`); ANSI 7 and 9..=15 are light backgrounds.
fn terminal_background(lookup: &dyn Fn(&str) -> Option<String>) -> Background {
    let bg =
        lookup("COLORFGBG").and_then(|v| v.rsplit(';').next().and_then(|b| b.parse::<u8>().ok()));
    match bg {
        Some(7 | 9..=15) => Background::Light,
        _ => Background::Dark,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let owned: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |key| owned.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
    }

    #[test]
    fn empty_environment_is_dark() {
        let theme = Theme::detect(&env(&[]));
        assert_eq!(theme, Theme::dark());
    }

    #[test]
    fn no_color_selects_mono() {
        let theme = Theme::detect(&env(&[("NO_COLOR", "1")]));
        assert_eq!(theme, Theme::mono());
    }

    #[test]
    fn empty_no_color_is_ignored() {
        let theme = Theme::detect(&env(&[("NO_COLOR", "")]));
        assert_eq!(theme, Theme::dark());
    }

    #[test]
    fn dumb_terminal_selects_mono() {
        let theme = Theme::detect(&env(&[("TERM", "dumb")]));
        // A dumb terminal also gets the ASCII glyphs.
        assert_eq!(
            theme,
            Theme {
                glyphs: Glyphs::ASCII,
                ..Theme::mono()
            }
        );
    }

    #[test]
    fn light_background_selects_light() {
        let theme = Theme::detect(&env(&[("COLORFGBG", "0;15")]));
        assert_eq!(theme, Theme::light());
    }

    #[test]
    fn dark_background_selects_dark() {
        let theme = Theme::detect(&env(&[("COLORFGBG", "15;0")]));
        assert_eq!(theme, Theme::dark());
    }

    #[test]
    fn explicit_theme_beats_no_color() {
        let theme = Theme::detect(&env(&[("SKILLZ_THEME", "light"), ("NO_COLOR", "1")]));
        assert_eq!(theme, Theme::light());
    }

    #[test]
    fn unknown_explicit_theme_falls_back_to_detection() {
        let theme = Theme::detect(&env(&[("SKILLZ_THEME", "neon"), ("NO_COLOR", "1")]));
        assert_eq!(theme, Theme::mono());
    }

    #[test]
    fn mono_marks_the_cursor_row_with_a_prompt() {
        assert_eq!(Theme::mono().glyphs.cursor, Glyphs::ASCII.cursor);
    }

    #[test]
    fn an_ascii_locale_swaps_the_glyphs_but_keeps_the_colours() {
        let theme = Theme::detect(&env(&[("LANG", "C")]));

        assert_eq!(theme.glyphs, Glyphs::ASCII);
        assert_eq!(theme.accent, Theme::dark().accent);
    }

    #[test]
    fn mono_uses_no_colours() {
        let m = Theme::mono();
        let styles = [m.accent, m.muted, m.warn, m.danger, m.selection, m.title];
        assert!(styles.iter().all(|s| s.fg.is_none() && s.bg.is_none()));
    }

    #[test]
    fn state_style_maps_each_state_to_its_role() {
        let t = Theme::dark();
        assert_eq!(t.state_style(State::Ok), t.ok);
        assert_eq!(t.state_style(State::Warn), t.warn);
        assert_eq!(t.state_style(State::Danger), t.danger);
        assert_eq!(t.state_style(State::Muted), t.muted);
    }

    #[test]
    fn every_kind_has_its_own_colour_except_in_mono() {
        let kinds = [
            ItemKind::Skill,
            ItemKind::Plugin,
            ItemKind::Subagent,
            ItemKind::Mcp,
            ItemKind::Hook,
        ];
        let colours = |t: Theme| -> Vec<_> { kinds.map(|k| t.kind_style(k).fg).to_vec() };
        for theme in [Theme::dark(), Theme::light()] {
            let all = colours(theme);
            assert!(all.iter().all(Option::is_some));
            assert!(all.iter().enumerate().all(|(i, c)| !all[..i].contains(c)));
        }
        assert!(colours(Theme::mono()).iter().all(Option::is_none));
    }

    #[test]
    fn focused_border_differs_from_unfocused() {
        let t = Theme::dark();
        assert_ne!(t.border_for(true), t.border_for(false));
    }
}
