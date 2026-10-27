//! Every key binding in one table: the keys that mean an `Action`, in which `Scope`, and how
//! the key bar and the `?` overlay describe them. Screens match on `Action`, never on keys.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Navigation and leaving, valid wherever a list has the keys.
    Common,
    /// The single list of plugins, each installed or not in the project.
    Plugins,
    /// Choosing library items with checkboxes.
    Picker,
    Tree,
    /// While a filter is being typed, so letters stay text.
    Filter,
    Plan,
    Result,
    Confirm,
}

impl Scope {
    pub fn title(self) -> &'static str {
        match self {
            Scope::Common => "Everywhere",
            Scope::Plugins => "Plugins",
            Scope::Picker => "Choose items",
            Scope::Tree => "List",
            Scope::Filter => "Filter",
            Scope::Plan => "Plan",
            Scope::Result => "Result",
            Scope::Confirm => "Confirm",
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
    Left,
    Right,
    Toggle,
    Mark,
    Info,
    ToggleDryRun,
    ToggleSteps,
    Copy,
    Retry,
    Yes,
    No,
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
use Scope::{Common, Confirm, Filter, Picker, Plan, Plugins, Result, Tree};

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
    bind(Plugins, A::Toggle, &[ch(' ')], "space", "select", 1),
    bind(Plugins, A::Select, &[plain(KeyCode::Enter)], "enter", "apply", 2),
    bind(Plugins, A::Back, &[plain(KeyCode::Esc)], "esc", "clear selection", 0),
    bind(Plugins, A::Info, &[ch('i')], "i", "info", 3),
    bind(Plugins, A::Filter, &[ch('/')], "/", "filter", 4),
    bind(Picker, A::Toggle, &[ch(' ')], "space", "check", 0),
    bind(Picker, A::Select, &[plain(KeyCode::Enter)], "enter", "save selection", 0),
    bind(Picker, A::Left, &[plain(KeyCode::Left), ch('h')], "← →", "collapse / expand", 0),
    bind(Picker, A::Right, &[plain(KeyCode::Right), ch('l')], "", "", 0),
    bind(Tree, A::Mark,&[ch('x'), plain(KeyCode::Tab)], "x tab", "mark", 1),
    bind(Tree, A::Select, &[plain(KeyCode::Enter)], "enter", "actions", 2),
    bind(Tree, A::Filter, &[ch('/')], "/", "filter", 3),
    bind(Tree, A::Info, &[ch('i')], "i", "info", 4),
    bind(Tree, A::Toggle, &[ch(' ')], "space", "expand", 5),
    bind(Tree, A::Left, &[plain(KeyCode::Left), ch('h')], "← →", "collapse / expand", 0),
    bind(Tree, A::Right, &[plain(KeyCode::Right), ch('l')], "", "", 0),
    bind(Plan, A::Select, &[plain(KeyCode::Enter)], "enter", "choose", 0),
    bind(Plan, A::Yes, &[ch('y'), ch('Y')], "y", "apply", 0),
    bind(Plan, A::Back, &[plain(KeyCode::Esc)], "esc", "cancel", 0),
    bind(Plan, A::No, &[ch('n'), ch('N')], "n", "cancel", 0),
    bind(Plan, A::Left, &[plain(KeyCode::Left), ch('h'), plain(KeyCode::BackTab)], "← →", "change button", 0),
    bind(Plan, A::Right, &[plain(KeyCode::Right), ch('l'), plain(KeyCode::Tab)], "", "", 0),
    bind(Plan, A::ToggleDryRun, &[ch('d')], "d", "dry run", 0),
    bind(Plan, A::Toggle, &[ch(' ')], "space", "check", 0),
    bind(Plan, A::ToggleSteps, &[ch('s')], "s", "steps", 0),
    bind(Result, A::Select, &[plain(KeyCode::Enter), ch(' ')], "enter", "choose", 1),
    bind(Result, A::Left, &[plain(KeyCode::Left), ch('h'), plain(KeyCode::BackTab)], "← →", "change button", 0),
    bind(Result, A::Right, &[plain(KeyCode::Right), ch('l'), plain(KeyCode::Tab)], "", "", 0),
    bind(Result, A::Back, &[plain(KeyCode::Esc)], "esc", "close", 0),
    bind(Result, A::Copy, &[ch('c')], "c", "copy", 2),
    bind(Result, A::Retry, &[ch('r')], "r", "retry", 3),
    bind(Confirm, A::Yes, &[ch('y'), ch('Y')], "y", "yes", 1),
    bind(Confirm, A::No, &[ch('n'), ch('N')], "n", "no", 2),
    bind(Confirm, A::Toggle, &[plain(KeyCode::Tab), plain(KeyCode::BackTab), plain(KeyCode::Left), plain(KeyCode::Right)], "tab", "change answer", 0),
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
    resolve_in(&[scope], key)
}

/// What `key` means in the first of `scopes` that binds it, else in the common bindings.
/// `Scope::Filter` is exclusive: typed characters are never navigation there.
pub fn resolve_in(scopes: &[Scope], key: KeyEvent) -> Option<Action> {
    scopes
        .iter()
        .find_map(|scope| {
            BINDINGS
                .iter()
                .find(|b| b.scope == *scope && b.matches(key))
        })
        .or_else(|| {
            (!scopes.contains(&Filter))
                .then(|| {
                    BINDINGS
                        .iter()
                        .find(|b| b.scope == Common && b.matches(key))
                })
                .flatten()
        })
        .map(|binding| binding.action)
}

/// The key bar's hints for `scopes`, most important first.
pub fn hints(scopes: &[Scope]) -> Vec<(&'static str, &'static str)> {
    hint_bindings(scopes)
        .into_iter()
        .map(|b| (b.keys, b.label))
        .collect()
}

pub fn hint_bindings(scopes: &[Scope]) -> Vec<&'static Binding> {
    let mut ranked: Vec<&Binding> = BINDINGS
        .iter()
        .filter(|b| b.rank > 0 && (b.scope == Common || scopes.contains(&b.scope)))
        .collect();
    ranked.sort_by_key(|b| b.rank);
    ranked
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
        let plugins = press(Plugins, 'i');
        let result = press(Result, 'i');
        let tree = press(Tree, 'x');

        assert_eq!(
            (plugins, result, tree),
            (Some(Action::Info), None, Some(Action::Mark))
        );
    }

    #[test]
    fn a_scope_falls_back_to_the_common_bindings() {
        assert_eq!(press(Tree, 'j'), Some(Action::Down));
        assert_eq!(press(Plugins, 'q'), Some(Action::Quit));
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

        assert_eq!(half, Some(Action::HalfPageDown));
        assert_eq!(press(Plugins, 'd'), None);
        assert_eq!(resolve(Plugins, ctrl_key('c')), Some(Action::Interrupt));
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
        for scope in [Plugins, Tree] {
            let hints = hints(&[scope]);
            let mut sorted = hints.clone();
            sorted.dedup();

            assert_eq!(hints, sorted);
            assert!(!hints.is_empty());
        }
        let ranks: Vec<u8> = BINDINGS
            .iter()
            .filter(|b| b.scope == Plugins && b.rank > 0)
            .map(|b| b.rank)
            .collect();
        let mut distinct = ranks.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(ranks.len(), distinct.len());
    }

    #[test]
    fn the_scopes_own_enter_replaces_the_common_one_in_help() {
        let selects = help_sections(&[Plugins])
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
        let sections = help_sections(&[Plugins]);

        let scopes: Vec<Scope> = sections.iter().map(|(s, _)| *s).collect();
        assert_eq!(scopes, [Plugins, Common]);
        assert!(
            sections
                .iter()
                .all(|(s, b)| b.iter().all(|b| b.scope == *s))
        );
    }

    #[test]
    fn caller_scope_order_wins_over_table_order() {
        let key = key(KeyCode::Tab);

        let plan_first = resolve_in(&[Plan, Confirm], key);
        let confirm_first = resolve_in(&[Confirm, Plan], key);

        // Assert overlapping bindings follow the scope order supplied by the caller.
        assert_eq!(plan_first, Some(Action::Right));
        assert_eq!(confirm_first, Some(Action::Toggle));
    }
}
