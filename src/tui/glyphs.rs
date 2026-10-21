//! The symbols screens draw state with, in a Unicode and an ASCII set.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyphs {
    /// Two columns, drawn before the row under the cursor.
    pub cursor: &'static str,
    pub loaded: &'static str,
    pub broken: &'static str,
    pub missing: &'static str,
    pub checked: &'static str,
    pub unchecked: &'static str,
    pub partial: &'static str,
    pub to_install: &'static str,
    pub to_remove: &'static str,
    pub open: &'static str,
    pub closed: &'static str,
}

impl Glyphs {
    pub const UNICODE: Glyphs = Glyphs {
        cursor: "▌ ",
        loaded: "✓",
        broken: "!",
        missing: "?",
        checked: "☑",
        unchecked: "☐",
        partial: "◩",
        to_install: "⊕",
        to_remove: "⊖",
        open: "▾",
        closed: "▸",
    };

    pub const ASCII: Glyphs = Glyphs {
        cursor: "> ",
        loaded: "+",
        broken: "!",
        missing: "?",
        checked: "X",
        unchecked: " ",
        partial: "-",
        to_install: "+",
        to_remove: "~",
        open: "-",
        closed: "+",
    };

    pub fn detect(lookup: &dyn Fn(&str) -> Option<String>) -> Self {
        if wants_ascii(lookup) {
            Glyphs::ASCII
        } else {
            Glyphs::UNICODE
        }
    }
}

fn wants_ascii(lookup: &dyn Fn(&str) -> Option<String>) -> bool {
    let forced = lookup("SKILLZ_ASCII").is_some_and(|v| !v.is_empty());
    let plain_terminal = lookup("TERM").is_some_and(|t| t == "linux" || t == "dumb");
    forced || plain_terminal || !speaks_utf8(lookup)
}

/// An unset locale is given the benefit of the doubt; a set one must name UTF-8.
fn speaks_utf8(lookup: &dyn Fn(&str) -> Option<String>) -> bool {
    let locale = ["LC_ALL", "LC_CTYPE", "LANG"]
        .into_iter()
        .find_map(|name| lookup(name).filter(|v| !v.is_empty()));
    locale.is_none_or(|l| {
        let l = l.to_ascii_lowercase();
        l.contains("utf-8") || l.contains("utf8")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        let owned = pairs.to_vec();
        move |key| {
            owned
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.to_string())
        }
    }

    #[test]
    fn an_empty_environment_gets_unicode() {
        assert_eq!(Glyphs::detect(&env(&[])), Glyphs::UNICODE);
    }

    #[test]
    fn a_utf8_locale_gets_unicode() {
        let lookup = env(&[("LANG", "en_US.UTF-8")]);
        assert_eq!(Glyphs::detect(&lookup), Glyphs::UNICODE);
    }

    #[test]
    fn a_non_utf8_locale_gets_ascii() {
        assert_eq!(Glyphs::detect(&env(&[("LANG", "C")])), Glyphs::ASCII);
    }

    #[test]
    fn the_first_set_locale_variable_decides() {
        let lookup = env(&[("LC_ALL", "C"), ("LANG", "en_US.UTF-8")]);
        assert_eq!(Glyphs::detect(&lookup), Glyphs::ASCII);
    }

    #[test]
    fn the_override_and_plain_terminals_get_ascii() {
        for pair in [("SKILLZ_ASCII", "1"), ("TERM", "linux"), ("TERM", "dumb")] {
            assert_eq!(Glyphs::detect(&env(&[pair])), Glyphs::ASCII);
        }
    }

    #[test]
    fn every_state_glyph_is_one_column_wide() {
        use unicode_width::UnicodeWidthStr;
        for set in [Glyphs::UNICODE, Glyphs::ASCII] {
            let all = [
                set.loaded,
                set.broken,
                set.missing,
                set.checked,
                set.unchecked,
                set.partial,
                set.to_install,
                set.to_remove,
                set.open,
                set.closed,
            ];

            // One column each keeps rows aligned whichever set is drawn.
            assert!(all.iter().all(|g| g.width() == 1));
            assert_eq!(set.cursor.width(), 2);
        }
    }
}
