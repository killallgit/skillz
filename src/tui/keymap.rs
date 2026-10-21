//! Every key binding in one table: the keys that mean an `Action`, in which `Scope`, and how
//! the key bar and the `?` overlay describe them. Screens match on `Action`, never on keys.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Navigation and leaving, valid wherever a list has the keys.
    Common,
    Home,
    Tree,
    /// While a filter is being typed, so letters stay text.
    Filter,
}

impl Scope {
    pub fn title(self) -> &'static str {
        match self {
            Scope::Common => "Everywhere",
            Scope::Home => "Workspace",
            Scope::Tree => "List",
            Scope::Filter => "Filter",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Up,
    Down,
    PageUp,
    PageDown,
    HalfPageUp,
    HalfPageDown,
    Top,
    Bottom,
    Select,
    Back,
    Quit,
    Interrupt,
    Help,
    Filter,
    SwitchPane,
    DetailUp,
    DetailDown,
    NewProfile,
    SaveProfile,
    AddInstalled,
    AddTyped,
    RemoveEntries,
    Delete,
    Left,
    Right,
    Toggle,
    Mark,
    Info,
}

#[derive(Debug, Clone, Copy)]
struct Chord(KeyCode, KeyModifiers);

const fn plain(code: KeyCode) -> Chord {
    Chord(code, KeyModifiers::NONE)
}

const fn ch(c: char) -> Chord {
    plain(KeyCode::Char(c))
}

const fn ctrl(c: char) -> Chord {
    Chord(KeyCode::Char(c), KeyModifiers::CONTROL)
}

#[derive(Debug, Clone, Copy)]
pub struct Binding {
    pub scope: Scope,
    pub action: Action,
    chords: &'static [Chord],
    /// How the keys read in help and on the key bar.
    pub keys: &'static str,
    /// Empty for a binding that shares the previous row's description.
    pub label: &'static str,
    /// Position on the key bar, lowest first; 0 keeps it off the bar.
    pub rank: u8,
}

const fn bind(
    scope: Scope,
    action: Action,
    chords: &'static [Chord],
    keys: &'static str,
    label: &'static str,
    rank: u8,
) -> Binding {
    Binding {
        scope,
        action,
        chords,
        keys,
        label,
        rank,
    }
}

use Action as A;
use Scope::{Common, Filter, Home, Tree};

#[rustfmt::skip]
pub const BINDINGS: &[Binding] = &[
    bind(Common, A::Up, &[plain(KeyCode::Up), ch('k')], "↑ ↓ j k", "move", 30),
    bind(Common, A::Down, &[plain(KeyCode::Down), ch('j')], "", "", 0),
    bind(Common, A::PageUp, &[plain(KeyCode::PageUp)], "PgUp PgDn", "page", 0),
    bind(Common, A::PageDown, &[plain(KeyCode::PageDown)], "", "", 0),
    bind(Common, A::HalfPageUp, &[ctrl('u')], "^u ^d", "half page", 0),
    bind(Common, A::HalfPageDown, &[ctrl('d')], "", "", 0),
    bind(Common, A::Top, &[ch('g'), plain(KeyCode::Home)], "g G", "top / bottom", 0),
    bind(Common, A::Bottom, &[ch('G'), plain(KeyCode::End)], "", "", 0),
    bind(Common, A::Select, &[plain(KeyCode::Enter)], "enter", "choose", 0),
    bind(Common, A::Back, &[plain(KeyCode::Esc)], "esc", "close / back", 0),
    bind(Common, A::Quit, &[ch('q')], "q", "quit", 40),
    bind(Common, A::Interrupt, &[ctrl('c')], "^c", "quit anywhere", 0),
    bind(Common, A::Help, &[ch('?')], "?", "help", 0),
    bind(Home, A::SwitchPane, &[plain(KeyCode::Tab), plain(KeyCode::Left), plain(KeyCode::Right), ch('h'), ch('l')], "tab", "pane", 1),
    bind(Home, A::Select, &[plain(KeyCode::Enter)], "enter", "apply", 2),
    bind(Home, A::DetailUp, &[ch('[')], "[ ]", "scroll detail", 0),
    bind(Home, A::DetailDown, &[ch(']')], "", "", 0),
    bind(Home, A::NewProfile, &[ch('n')], "n", "new profile", 5),
    bind(Home, A::SaveProfile, &[ch('s')], "s", "save as profile", 6),
    bind(Home, A::AddInstalled, &[ch('i')], "i", "add installed", 7),
    bind(Home, A::AddTyped, &[ch('t')], "t", "add typed", 8),
    bind(Home, A::RemoveEntries, &[ch('r')], "r", "remove entries", 9),
    bind(Home, A::Delete, &[ch('d')], "d", "delete", 10),
    bind(Home, A::Filter, &[ch('/')], "/", "filter", 11),
    bind(Tree, A::Mark, &[ch('x'), plain(KeyCode::Tab)], "x tab", "mark", 1),
    bind(Tree, A::Select, &[plain(KeyCode::Enter)], "enter", "actions", 2),
    bind(Tree, A::Filter, &[ch('/')], "/", "filter", 3),
    bind(Tree, A::Info, &[ch('i')], "i", "info", 4),
    bind(Tree, A::Toggle, &[ch(' ')], "space", "expand", 5),
    bind(Tree, A::Left, &[plain(KeyCode::Left), ch('h')], "← →", "collapse / expand", 0),
    bind(Tree, A::Right, &[plain(KeyCode::Right), ch('l')], "", "", 0),
    bind(Filter, A::Up, &[plain(KeyCode::Up)], "", "", 0),
    bind(Filter, A::Down, &[plain(KeyCode::Down)], "", "", 0),
    bind(Filter, A::PageUp, &[plain(KeyCode::PageUp)], "", "", 0),
    bind(Filter, A::PageDown, &[plain(KeyCode::PageDown)], "", "", 0),
];

impl Chord {
    fn matches(&self, key: KeyEvent) -> bool {
        key.code == self.0 && key.modifiers - KeyModifiers::SHIFT == self.1
    }
}

impl Binding {
    fn matches(&self, key: KeyEvent) -> bool {
        self.chords.iter().any(|c| c.matches(key))
    }
}

/// What `key` means in `scope`; the scope's own bindings win over the common ones.
/// `Scope::Filter` is exclusive: typed characters are never navigation there.
pub fn resolve(scope: Scope, key: KeyEvent) -> Option<Action> {
    let own = |b: &&Binding| b.scope == scope && b.matches(key);
    let common = |b: &&Binding| scope != Scope::Filter && b.scope == Common && b.matches(key);
    BINDINGS
        .iter()
        .find(own)
        .or_else(|| BINDINGS.iter().find(common))
        .map(|b| b.action)
}

/// How `action` is written in `scope`, for hints that name a key.
pub fn keys_for(scope: Scope, action: Action) -> &'static str {
    BINDINGS
        .iter()
        .find(|b| b.scope == scope && b.action == action)
        .map_or("", |b| b.keys)
}

/// The key bar's hints for `scopes`, most important first.
pub fn hints(scopes: &[Scope]) -> Vec<(&'static str, &'static str)> {
    let mut ranked: Vec<&Binding> = BINDINGS
        .iter()
        .filter(|b| b.rank > 0 && (b.scope == Common || scopes.contains(&b.scope)))
        .collect();
    ranked.sort_by_key(|b| b.rank);
    ranked.iter().map(|b| (b.keys, b.label)).collect()
}

fn overridden(binding: &Binding, scopes: &[Scope]) -> bool {
    binding.scope == Common
        && BINDINGS
            .iter()
            .any(|b| scopes.contains(&b.scope) && b.action == binding.action)
}

/// The rows of the `?` overlay: each scope's own bindings in the order given, then the common
/// ones the scopes do not replace.
pub fn help_sections(scopes: &[Scope]) -> Vec<(Scope, Vec<&'static Binding>)> {
    let listed = |scope: Scope| {
        let bindings = BINDINGS
            .iter()
            .filter(|b| b.scope == scope && !b.label.is_empty())
            .filter(|b| scope != Common || !overridden(b, scopes))
            .collect();
        (scope, bindings)
    };
    scopes.iter().copied().chain([Common]).map(listed).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::{ctrl as ctrl_key, key};

    fn press(scope: Scope, c: char) -> Option<Action> {
        resolve(scope, key(KeyCode::Char(c)))
    }

    #[test]
    fn vim_and_arrow_keys_mean_the_same_move() {
        let up = resolve(Common, key(KeyCode::Up));
        let k = press(Common, 'k');

        // Both spellings of "up" resolve to one action.
        assert_eq!((up, k), (Some(Action::Up), Some(Action::Up)));
    }

    #[test]
    fn the_same_letter_means_different_things_per_scope() {
        let home = press(Home, 'i');
        let tree = press(Tree, 'i');

        assert_eq!(
            (home, tree),
            (Some(Action::AddInstalled), Some(Action::Info))
        );
    }

    #[test]
    fn a_scope_falls_back_to_the_common_bindings() {
        assert_eq!(press(Tree, 'j'), Some(Action::Down));
        assert_eq!(press(Home, 'q'), Some(Action::Quit));
    }

    #[test]
    fn shifted_letters_resolve_with_or_without_the_shift_flag() {
        let shifted = KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT);

        let both = (resolve(Common, shifted), press(Common, 'G'));

        assert_eq!(both, (Some(Action::Bottom), Some(Action::Bottom)));
    }

    #[test]
    fn control_chords_are_distinct_from_the_plain_letter() {
        let half = resolve(Common, ctrl_key('d'));
        let delete = press(Home, 'd');

        assert_eq!(half, Some(Action::HalfPageDown));
        assert_eq!(delete, Some(Action::Delete));
        assert_eq!(resolve(Home, ctrl_key('c')), Some(Action::Interrupt));
    }

    #[test]
    fn typing_a_filter_never_navigates_on_letters() {
        let letters = ['j', 'k', 'q', 'g', 'x'].map(|c| press(Filter, c));

        assert_eq!(letters, [None; 5]);
        assert_eq!(
            resolve(Filter, key(KeyCode::Down)),
            Some(Action::Down),
            "arrows still move the list"
        );
    }

    #[test]
    fn hints_come_out_most_important_first_and_without_duplicates() {
        for scope in [Home, Tree] {
            let hints = hints(&[scope]);
            let mut sorted = hints.clone();
            sorted.dedup();

            assert_eq!(hints, sorted);
            assert!(!hints.is_empty());
        }
        let ranks: Vec<u8> = BINDINGS
            .iter()
            .filter(|b| b.scope == Home && b.rank > 0)
            .map(|b| b.rank)
            .collect();
        assert!(ranks.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn the_scopes_own_enter_replaces_the_common_one_in_help() {
        let selects = help_sections(&[Home])
            .into_iter()
            .flat_map(|(_, bindings)| bindings)
            .filter(|b| b.action == Action::Select);

        assert_eq!(selects.count(), 1);
    }

    #[test]
    fn every_listed_binding_names_its_keys() {
        let nameless = BINDINGS
            .iter()
            .filter(|b| !b.label.is_empty() && b.keys.is_empty());

        assert_eq!(nameless.count(), 0);
    }

    #[test]
    fn help_lists_the_scope_first_then_the_common_bindings() {
        let sections = help_sections(&[Home]);

        let scopes: Vec<Scope> = sections.iter().map(|(s, _)| *s).collect();
        assert_eq!(scopes, [Home, Common]);
        assert!(
            sections
                .iter()
                .all(|(s, b)| b.iter().all(|b| b.scope == *s))
        );
    }
}
