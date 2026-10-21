use ratatui::crossterm::event::KeyCode;

use super::rows::tests::skill;
use super::*;
use crate::library::LibraryItem;
use crate::model::{Agent, Augmentation, Installer, Kind, Scope as Level};
use crate::tui::widgets::testing::{key, keys, render_to_buffer};

fn installed(name: &str) -> Augmentation {
    Augmentation {
        hooks: vec![],
        id: format!("skill:{name}"),
        name: name.into(),
        kind: Kind::Skill,
        agents: vec![Agent::Claude, Agent::Codex],
        scope: Level::Project,
        group: "o/r".into(),
        source: Some("o/r".into()),
        installer: Installer::Npx,
        origin: Default::default(),
        children: vec![],
        removal: vec![],
    }
}

fn profile(name: &str, skills: &[&str]) -> Profile {
    let mut profile = Profile::new(name);
    profile.entries = skills.iter().map(|s| skill(s)).collect();
    profile
}

fn model() -> Model {
    Model {
        dir: "~/app".into(),
        installed: vec![installed("a")],
        global: vec![],
        library: Catalog::from_items(vec![
            LibraryItem::from_entry(skill("a")),
            LibraryItem::from_entry(skill("b")),
        ]),
        profiles: vec![profile("api", &["a"]), profile("web", &["a", "b"])],
        notice: None,
    }
}

fn loaded() -> ProfilesTab {
    let mut tab = ProfilesTab::new();
    tab.load(&model());
    tab
}

fn press(tab: &mut ProfilesTab, code: KeyCode) -> Option<Act> {
    tab.handle(key(code))
}

fn type_in(tab: &mut ProfilesTab, text: &str) {
    keys(text).into_iter().for_each(|k| {
        tab.handle(k);
    });
}

fn char_key(tab: &mut ProfilesTab, c: char) -> Option<Act> {
    press(tab, KeyCode::Char(c))
}

#[test]
fn each_profile_counts_what_the_project_lacks_of_it() {
    let tab = loaded();

    assert_eq!(tab.missing, [0, 1]);
}

#[test]
fn the_entries_follow_the_profile_under_the_cursor() {
    let mut tab = loaded();
    let api = tab.rows.len();

    press(&mut tab, KeyCode::Down);

    assert_eq!((api, tab.rows.len()), (2, 3));
}

#[test]
fn enter_and_a_apply_additively_and_e_applies_exactly() {
    let mut tab = loaded();

    let enter = press(&mut tab, KeyCode::Enter);
    let a = char_key(&mut tab, 'a');
    let exact = char_key(&mut tab, 'e');

    let apply = Some(Act::Apply {
        profile: "api".into(),
    });
    assert_eq!((enter, a), (apply.clone(), apply));
    assert_eq!(
        exact,
        Some(Act::ApplyExact {
            profile: "api".into()
        })
    );
}

#[test]
fn applying_with_no_profiles_asks_for_nothing() {
    let mut tab = ProfilesTab::new();

    assert_eq!(press(&mut tab, KeyCode::Enter), None);
    assert_eq!(char_key(&mut tab, 'e'), None);
}

#[test]
fn n_asks_for_a_name_then_creates_the_profile() {
    let mut tab = loaded();

    char_key(&mut tab, 'n');
    let capturing = tab.captures_keys();
    type_in(&mut tab, "docs");
    let act = press(&mut tab, KeyCode::Enter);

    assert!(capturing && !tab.captures_keys());
    assert_eq!(
        act,
        Some(Act::NewProfile {
            name: "docs".into()
        })
    );
}

#[test]
fn s_asks_for_a_name_then_saves_the_project() {
    let mut tab = loaded();

    char_key(&mut tab, 's');
    type_in(&mut tab, "snap");
    let act = press(&mut tab, KeyCode::Enter);

    assert_eq!(
        act,
        Some(Act::SaveCurrent {
            name: "snap".into()
        })
    );
}

#[test]
fn escape_leaves_the_name_prompt_without_an_act() {
    let mut tab = loaded();
    char_key(&mut tab, 'n');

    let act = press(&mut tab, KeyCode::Esc);

    assert_eq!((act, tab.captures_keys()), (None, false));
}

#[test]
fn building_writes_the_checked_library_items_into_the_profile() {
    let mut tab = loaded();

    char_key(&mut tab, 'b');
    // Section, a (checked), b: check b.
    [KeyCode::Down, KeyCode::Down, KeyCode::Char(' ')]
        .into_iter()
        .for_each(|code| {
            press(&mut tab, code);
        });
    let act = press(&mut tab, KeyCode::Enter);

    assert_eq!(
        act,
        Some(Act::SetEntries {
            profile: "api".into(),
            entries: vec![skill("a"), skill("b")]
        })
    );
}

#[test]
fn deleting_needs_a_yes_and_defaults_to_no() {
    let mut tab = loaded();

    char_key(&mut tab, 'x');
    let declined = press(&mut tab, KeyCode::Enter);
    char_key(&mut tab, 'x');
    let confirmed = char_key(&mut tab, 'y');

    assert_eq!(declined, None);
    assert_eq!(confirmed, Some(Act::DeleteProfile { name: "api".into() }));
}

#[test]
fn the_filter_narrows_the_profiles_and_the_entries_follow() {
    let mut tab = loaded();

    char_key(&mut tab, '/');
    type_in(&mut tab, "web");
    let typing = tab.prompt().is_some();
    press(&mut tab, KeyCode::Enter);

    assert!(typing && tab.prompt().is_none());
    assert_eq!(tab.list.visible(), [1]);
    assert_eq!(tab.rows.len(), 3);
}

#[test]
fn right_moves_into_the_entries_where_space_collapses_a_section() {
    let mut tab = loaded();

    press(&mut tab, KeyCode::Right);
    char_key(&mut tab, ' ');

    assert_eq!(tab.entries.visible().len(), 1);
    press(&mut tab, KeyCode::Left);
    assert_eq!(tab.focus, Focus::List);
}

#[test]
fn a_new_snapshot_keeps_the_cursor_on_the_same_row() {
    let mut tab = loaded();
    press(&mut tab, KeyCode::Down);

    tab.load(&model());

    assert_eq!(tab.current_name().as_deref(), Some("web"));
}

fn text_of(tab: &mut ProfilesTab) -> String {
    let theme = Theme::dark();
    let buffer = render_to_buffer(80, 24, |f| tab.draw(f, f.area(), &theme));
    buffer.content().iter().map(|c| c.symbol()).collect()
}

#[test]
fn drawing_shows_every_profile_name_and_the_current_entries() {
    let mut tab = loaded();

    let screen = text_of(&mut tab);

    assert!(
        ["api", "web", "skills a"]
            .iter()
            .all(|s| screen.contains(s))
    );
}

#[test]
fn drawing_an_empty_tab_does_not_panic() {
    let mut tab = ProfilesTab::new();

    text_of(&mut tab);
}
