//! Every key binding in one table: the keys that mean an `Action`, in which `Scope`, and how
//! the key bar and the `?` overlay describe them. Screens match on `Action`, never on keys.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Navigation and leaving, valid wherever a list has the keys.
    Common,
    /// The tab bar and what works on every tab.
    Workspace,
    /// The Project and Global tabs: the installed items and what the library could add.
    Installed,
    /// What only the Project tab has: comparing and applying a profile.
    Project,
    Library,
    Profiles,
    /// Choosing library items with checkboxes.
    Picker,
    Tree,
    /// While a filter is being typed, so letters stay text.
    Filter,
}

impl Scope {
    pub fn title(self) -> &'static str {
        match self {
            Scope::Common => "Everywhere",
            Scope::Workspace => "Tabs",
            Scope::Installed => "Installed",
            Scope::Project => "Profile",
            Scope::Library => "Library",
            Scope::Profiles => "Profiles",
            Scope::Picker => "Choose items",
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
    Left,
    Right,
    Toggle,
    Mark,
    Info,
    NextTab,
    PrevTab,
    GoTab(u8),
    PickProfile,
    ApplyProfile,
    Add,
    Import,
    Delete,
    NewProfile,
    ApplyExact,
    SaveCurrent,
    Build,
    Update,
    Install,
    Remove,
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
use Scope::{Common, Filter, Installed, Library, Picker, Profiles, Project, Tree, Workspace};

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
    bind(Workspace, A::NextTab, &[plain(KeyCode::Tab)], "tab", "next tab", 20),
    bind(Workspace, A::PrevTab, &[plain(KeyCode::BackTab)], "shift-tab", "previous tab", 0),
    bind(Workspace, A::GoTab(0), &[ch('1')], "1 2 3 4", "go to tab", 0),
    bind(Workspace, A::GoTab(1), &[ch('2')], "", "", 0),
    bind(Workspace, A::GoTab(2), &[ch('3')], "", "", 0),
    bind(Workspace, A::GoTab(3), &[ch('4')], "", "", 0),
    bind(Installed, A::Toggle, &[ch(' ')], "space", "pick", 1),
    bind(Installed, A::Select, &[plain(KeyCode::Enter)], "enter", "apply picks / fold", 2),
    bind(Installed, A::Install, &[ch('i')], "i", "install", 3),
    bind(Installed, A::Remove, &[ch('r')], "r", "remove", 4),
    bind(Project, A::PickProfile, &[ch('p')], "p", "compare", 5),
    bind(Project, A::ApplyProfile, &[ch('a')], "a", "apply profile", 6),
    bind(Installed, A::Update, &[ch('u')], "u", "update", 7),
    bind(Installed, A::Filter, &[ch('/')], "/", "filter", 8),
    bind(Installed, A::Left, &[plain(KeyCode::Left), ch('h')], "← →", "collapse / expand", 0),
    bind(Installed, A::Right, &[plain(KeyCode::Right), ch('l')], "", "", 0),
    bind(Library, A::Add, &[ch('a')], "a", "add", 1),
    bind(Library, A::Import, &[ch('i')], "i", "sync installed", 2),
    bind(Library, A::Delete, &[ch('x')], "x", "remove", 3),
    bind(Library, A::Filter, &[ch('/')], "/", "filter", 4),
    bind(Library, A::Toggle, &[ch(' ')], "space", "expand", 0),
    bind(Library, A::Select, &[plain(KeyCode::Enter)], "enter", "expand", 0),
    bind(Library, A::Left, &[plain(KeyCode::Left), ch('h')], "← →", "collapse / expand", 0),
    bind(Library, A::Right, &[plain(KeyCode::Right), ch('l')], "", "", 0),
    bind(Profiles, A::ApplyProfile, &[plain(KeyCode::Enter), ch('a')], "enter a", "apply", 1),
    bind(Profiles, A::NewProfile, &[ch('n')], "n", "new", 2),
    bind(Profiles, A::Build, &[ch('b')], "b", "build", 3),
    bind(Profiles, A::ApplyExact, &[ch('e')], "e", "apply exactly", 4),
    bind(Profiles, A::SaveCurrent, &[ch('s')], "s", "save project", 5),
    bind(Profiles, A::Delete, &[ch('x')], "x", "delete", 6),
    bind(Profiles, A::Filter, &[ch('/')], "/", "filter", 7),
    bind(Profiles, A::Toggle, &[ch(' ')], "space", "expand", 0),
    bind(Profiles, A::Left, &[plain(KeyCode::Left), ch('h')], "← →", "switch pane", 0),
    bind(Profiles, A::Right, &[plain(KeyCode::Right), ch('l')], "", "", 0),
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
    let own = |b: &&Binding| scopes.contains(&b.scope) && b.matches(key);
    let common = |b: &&Binding| !scopes.contains(&Filter) && b.scope == Common && b.matches(key);
    BINDINGS
        .iter()
        .find(own)
        .or_else(|| BINDINGS.iter().find(common))
        .map(|b| b.action)
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
        let project = press(Project, 'p');
        let tree = press(Tree, 'p');

        assert_eq!((project, tree), (Some(Action::PickProfile), None));
    }

    #[test]
    fn a_scope_falls_back_to_the_common_bindings() {
        assert_eq!(press(Tree, 'j'), Some(Action::Down));
        assert_eq!(press(Project, 'q'), Some(Action::Quit));
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
        assert_eq!(press(Project, 'd'), None);
        assert_eq!(resolve(Project, ctrl_key('c')), Some(Action::Interrupt));
    }

    #[test]
    fn the_tab_keys_move_between_tabs_and_jump_by_number() {
        let shift_tab = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);

        let keys = [
            resolve(Workspace, key(KeyCode::Tab)),
            resolve(Workspace, shift_tab),
            press(Workspace, '1'),
            press(Workspace, '4'),
        ];

        assert_eq!(
            keys,
            [
                Some(Action::NextTab),
                Some(Action::PrevTab),
                Some(Action::GoTab(0)),
                Some(Action::GoTab(3))
            ]
        );
    }

    #[test]
    fn a_tab_binds_the_keys_of_all_its_scopes_and_the_global_tab_has_no_profile_keys() {
        let project = [Installed, Project];
        let keys = ['i', 'p', 'a'];

        let on_project = keys.map(|c| resolve_in(&project, key(KeyCode::Char(c))));
        let on_global = keys.map(|c| resolve_in(&[Installed], key(KeyCode::Char(c))));

        assert_eq!(
            on_project,
            [
                Some(Action::Install),
                Some(Action::PickProfile),
                Some(Action::ApplyProfile)
            ]
        );
        assert_eq!(on_global, [Some(Action::Install), None, None]);
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
        for scope in [Installed, Library, Tree] {
            let hints = hints(&[scope]);
            let mut sorted = hints.clone();
            sorted.dedup();

            assert_eq!(hints, sorted);
            assert!(!hints.is_empty());
        }
        let ranks: Vec<u8> = BINDINGS
            .iter()
            .filter(|b| matches!(b.scope, Installed | Project | Workspace) && b.rank > 0)
            .map(|b| b.rank)
            .collect();
        let mut distinct = ranks.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(ranks.len(), distinct.len());
    }

    #[test]
    fn the_scopes_own_enter_replaces_the_common_one_in_help() {
        let selects = help_sections(&[Installed, Project])
            .into_iter()
            .flat_map(|(_, bindings)| bindings)
            .filter(|b| b.action == Action::Select);

        assert_eq!(selects.count(), 1);
    }

    #[test]
    fn each_profile_key_means_its_own_action() {
        let keys = ['n', 'b', 'e', 's', 'x', 'a'].map(|c| press(Profiles, c));

        assert_eq!(
            keys,
            [
                Some(Action::NewProfile),
                Some(Action::Build),
                Some(Action::ApplyExact),
                Some(Action::SaveCurrent),
                Some(Action::Delete),
                Some(Action::ApplyProfile)
            ]
        );
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
        let sections = help_sections(&[Installed]);

        let scopes: Vec<Scope> = sections.iter().map(|(s, _)| *s).collect();
        assert_eq!(scopes, [Installed, Common]);
        assert!(
            sections
                .iter()
                .all(|(s, b)| b.iter().all(|b| b.scope == *s))
        );
    }
}
