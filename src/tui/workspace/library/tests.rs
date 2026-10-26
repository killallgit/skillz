use ratatui::crossterm::event::KeyCode;

use super::rows::tests::{agent, hook, skill};
use super::*;
use crate::app::library::{Catalog, LibraryItem};
use crate::tui::widgets::testing::key;
use crate::tui::widgets::testing::open_all;
use skillz_harness::model::{Agent, Scope};

fn loaded() -> LibraryTab {
    let mut tab = LibraryTab::new();
    let library = Catalog::from_items(vec![skill("o/r", "tdd"), hook("notify")]);
    tab.load(&Model {
        library,
        ..Model::default()
    });
    open_all(&mut tab.outline);
    tab
}

fn press(tab: &mut LibraryTab, code: KeyCode) -> Option<Act> {
    tab.handle(key(code))
}

fn type_text(tab: &mut LibraryTab, text: &str) {
    text.chars().for_each(|c| {
        press(tab, KeyCode::Char(c));
    });
}

/// Rows are the package o/r (0), tdd (1), the unowned hooks (2) and notify (3).
fn on_row(tab: &mut LibraryTab, row: usize) {
    (0..row).for_each(|_| {
        press(tab, KeyCode::Down);
    });
}

#[test]
fn a_typed_line_is_handed_over_as_typed_when_submitted() {
    let mut tab = loaded();

    press(&mut tab, KeyCode::Char('a'));
    type_text(&mut tab, "skill o/r grill");
    let act = press(&mut tab, KeyCode::Enter);

    assert_eq!(
        act,
        Some(Act::LibraryAdd {
            spec: "skill o/r grill".into()
        })
    );
    assert!(!tab.captures_keys());
}

#[test]
fn escape_or_an_empty_line_adds_nothing_and_closes_the_prompt() {
    let mut tab = loaded();

    press(&mut tab, KeyCode::Char('a'));
    let empty = press(&mut tab, KeyCode::Enter);
    press(&mut tab, KeyCode::Char('a'));
    type_text(&mut tab, "skill");
    let cancelled = press(&mut tab, KeyCode::Esc);

    assert_eq!((empty, cancelled), (None, None));
    assert!(!tab.captures_keys());
}

#[test]
fn the_prompt_takes_every_key_so_letters_are_text() {
    let mut tab = loaded();

    press(&mut tab, KeyCode::Char('a'));
    let act = press(&mut tab, KeyCode::Char('i'));

    assert_eq!(act, None);
    assert!(tab.captures_keys());
}

/// Rows are the same as in `loaded`; the tools offered are Claude and Codex.
fn install_with(keys: &[KeyCode], entry: Spec) -> (Option<Act>, bool) {
    let mut tab = LibraryTab::new();
    tab.load(&Model {
        library: Catalog::from_items(vec![LibraryItem::from_entry(entry)]),
        agents: vec![Agent::Claude, Agent::Codex],
        ..Model::default()
    });
    open_all(&mut tab.outline);
    on_row(&mut tab, 1);
    let acts: Vec<_> = keys.iter().map(|code| press(&mut tab, *code)).collect();
    (acts.into_iter().flatten().last(), tab.captures_keys())
}

fn tdd(agents: Vec<Agent>) -> Spec {
    Spec::Skill {
        source: "o/r".into(),
        names: vec!["tdd".into()],
        agents,
    }
}

fn install_act(scope: Scope, agents: Vec<Agent>) -> Option<Act> {
    Some(Act::LibraryInstall {
        scope,
        entry: tdd(agents),
    })
}

#[test]
fn i_on_an_item_opens_a_dialog_and_enter_installs_in_the_project_for_every_tool() {
    let (opened, still_open) = install_with(&[KeyCode::Char('i')], tdd(vec![]));
    let (act, closed) = install_with(&[KeyCode::Char('i'), KeyCode::Enter], tdd(vec![]));

    assert_eq!((opened, still_open), (None, true));
    assert_eq!(
        (act, closed),
        (
            install_act(Scope::Project, vec![Agent::Claude, Agent::Codex]),
            false
        )
    );
}

#[test]
fn the_dialog_picks_the_scope_and_switches_each_tool() {
    use KeyCode::{Char, Down, Enter};
    let keys = [
        Char('i'),
        Down,
        Char(' '), // Global
        Down,
        Down,
        Char(' '), // Codex off
        Enter,
    ];

    let (act, _) = install_with(&keys, tdd(vec![]));

    assert_eq!(act, install_act(Scope::Global, vec![Agent::Claude]));
}

#[test]
fn an_entry_that_names_a_tool_starts_with_only_that_tool_checked() {
    let (act, _) = install_with(
        &[KeyCode::Char('i'), KeyCode::Enter],
        tdd(vec![Agent::Codex]),
    );

    assert_eq!(act, install_act(Scope::Project, vec![Agent::Codex]));
}

#[test]
fn nothing_is_installed_while_no_tool_is_checked() {
    use KeyCode::{Char, Down, Enter};
    let keys = [Char('i'), Down, Down, Char(' '), Down, Char(' '), Enter];

    let (act, still_open) = install_with(&keys, tdd(vec![]));

    assert_eq!((act, still_open), (None, true));
}

#[test]
fn escape_closes_the_dialog_without_installing() {
    let (act, still_open) = install_with(&[KeyCode::Char('i'), KeyCode::Esc], tdd(vec![]));

    assert_eq!((act, still_open), (None, false));
}

#[test]
fn i_on_a_package_opens_nothing() {
    let mut tab = loaded();

    press(&mut tab, KeyCode::Char('i'));

    assert!(!tab.captures_keys());
}

#[test]
fn removing_an_item_needs_a_yes() {
    let mut tab = loaded();
    on_row(&mut tab, 1);

    press(&mut tab, KeyCode::Char('x'));
    let asked = tab.captures_keys();
    let act = press(&mut tab, KeyCode::Char('y'));

    assert!(asked);
    assert_eq!(act, Some(Act::LibraryRemove(skill("o/r", "tdd").entry)));
}

#[test]
fn declining_or_removing_a_package_removes_nothing() {
    let mut tab = loaded();

    press(&mut tab, KeyCode::Char('x'));
    let on_package = tab.captures_keys();
    on_row(&mut tab, 1);
    press(&mut tab, KeyCode::Char('x'));
    let declined = press(&mut tab, KeyCode::Char('n'));

    assert!(!on_package);
    assert_eq!(declined, None);
    assert!(!tab.captures_keys());
}

#[test]
fn space_on_a_package_folds_it_and_hides_its_items() {
    let mut tab = loaded();
    let before = tab.outline.visible().len();

    press(&mut tab, KeyCode::Char(' '));

    assert_eq!((before, tab.outline.visible().len()), (4, 3));
}

#[test]
fn a_filter_narrows_the_rows_to_what_matches() {
    let mut tab = loaded();

    press(&mut tab, KeyCode::Char('/'));
    type_text(&mut tab, "notify");
    press(&mut tab, KeyCode::Enter);

    assert!(tab.outline.is_filtered());
    assert_eq!(tab.outline.visible(), [2, 3]);
}

#[test]
fn grouped_agents_can_be_folded_filtered_and_removed_individually() {
    let reviewer = agent("/src/team", "reviewer");
    let mut tab = LibraryTab::new();
    tab.load(&Model {
        library: Catalog::from_items(vec![reviewer.clone(), agent("/src/team", "architect")]),
        ..Model::default()
    });
    open_all(&mut tab.outline);

    press(&mut tab, KeyCode::Char(' '));
    assert_eq!(tab.outline.visible(), [0]);
    press(&mut tab, KeyCode::Enter);
    assert_eq!(tab.outline.visible(), [0, 1]);
    press(&mut tab, KeyCode::Down);
    press(&mut tab, KeyCode::Right);
    assert_eq!(tab.outline.visible(), [0, 1, 2, 3]);

    press(&mut tab, KeyCode::Char('/'));
    type_text(&mut tab, "reviewer");
    press(&mut tab, KeyCode::Enter);
    assert_eq!(tab.outline.visible(), [0, 1, 3]);
    tab.outline.select_row(3);
    press(&mut tab, KeyCode::Char('x'));
    assert_eq!(
        press(&mut tab, KeyCode::Char('y')),
        Some(Act::LibraryRemove(reviewer.entry))
    );
}

#[test]
fn the_install_dialog_draws_at_small_and_large_sizes() {
    use crate::tui::widgets::testing::render_to_buffer;
    let theme = Theme::dark();
    for (width, height) in [(100, 30), (50, 15), (30, 10)] {
        let mut tab = LibraryTab::new();
        tab.load(&Model {
            library: Catalog::from_items(vec![skill("o/r", "tdd")]),
            agents: vec![Agent::Claude, Agent::Codex],
            ..Model::default()
        });
        open_all(&mut tab.outline);
        on_row(&mut tab, 1);
        press(&mut tab, KeyCode::Char('i'));

        render_to_buffer(width, height, |frame| tab.draw(frame, frame.area(), &theme));

        assert!(tab.captures_keys(), "{width}x{height}");
    }
}
