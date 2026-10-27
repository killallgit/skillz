use ratatui::crossterm::event::KeyCode;

use super::rows::tests::skill;
use super::*;
use crate::app::library::LibraryItem;
use crate::tui::widgets::testing::open_all;
use crate::tui::widgets::testing::{key, keys, render_to_buffer};
use skillz_harness::model::{Agent, Installed, Installer, Kind, Scope as Level};

fn installed(name: &str) -> Installed {
    Installed {
        details: Default::default(),
        hooks: vec![],
        members: Vec::new(),
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
        agents: vec![],
        library: Catalog::from_items(vec![
            LibraryItem::from_entry(skill("a")),
            LibraryItem::from_entry(skill("b")),
        ]),
        profiles: vec![profile("api", &["a"]), profile("web", &["a", "b"])],
        applied: None,
        applied_global: None,
        notice: None,
    }
}

fn loaded() -> ProfilesTab {
    let mut tab = ProfilesTab::new();
    tab.load(&model());
    open_all(&mut tab.list);
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

    // Assert the comparison stays attached to its profile.
    assert_eq!(
        tab.profiles
            .iter()
            .map(|row| row.missing)
            .collect::<Vec<_>>(),
        [0, 1]
    );
}

#[test]
fn each_profile_is_followed_by_its_entries_in_one_list() {
    let tab = loaded();

    // api, its one entry, web, its two.
    assert_eq!(tab.list.visible(), [0, 1, 2, 3, 4]);
}

#[test]
fn enter_and_e_open_the_editor_and_a_and_s_do_nothing() {
    let mut tab = loaded();

    let ignored = (char_key(&mut tab, 'a'), char_key(&mut tab, 's'));
    let idle = tab.captures_keys();
    press(&mut tab, KeyCode::Enter);
    let editing = tab.captures_keys();

    assert_eq!(ignored, (None, None));
    assert!(!idle && editing);
}

#[test]
fn editing_with_no_profiles_opens_nothing() {
    let mut tab = ProfilesTab::new();

    press(&mut tab, KeyCode::Enter);
    char_key(&mut tab, 'e');

    assert!(!tab.captures_keys());
}

#[test]
fn n_opens_one_modal_that_names_and_fills_the_new_profile() {
    let mut tab = loaded();

    char_key(&mut tab, 'n');
    let capturing = tab.captures_keys();
    type_in(&mut tab, "docs");
    // Move to the list, open past the section, check the first library skill, save.
    press(&mut tab, KeyCode::Tab);
    [KeyCode::Right, KeyCode::Down, KeyCode::Char(' ')]
        .into_iter()
        .for_each(|code| {
            press(&mut tab, code);
        });
    let act = char_key(&mut tab, 's');

    assert!(capturing && !tab.captures_keys());
    assert_eq!(
        act,
        Some(Act::CreateProfile {
            name: "docs".into(),
            entries: vec![skill("a")]
        })
    );
}

#[test]
fn enter_in_the_name_saves_the_new_profile_and_a_blank_name_saves_nothing() {
    let mut tab = loaded();
    char_key(&mut tab, 'n');

    let blank = press(&mut tab, KeyCode::Enter);
    type_in(&mut tab, "docs");
    let named = press(&mut tab, KeyCode::Enter);

    assert_eq!(blank, None);
    assert_eq!(
        named,
        Some(Act::CreateProfile {
            name: "docs".into(),
            entries: vec![]
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
fn editing_writes_the_checked_library_items_into_the_profile() {
    let mut tab = loaded();

    char_key(&mut tab, 'e');
    // The section, a (checked), b: check b.
    [
        KeyCode::Right,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Char(' '),
    ]
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
fn the_filter_narrows_the_list() {
    let mut tab = loaded();

    char_key(&mut tab, '/');
    type_in(&mut tab, "web");
    let typing = tab.prompt().is_some();
    press(&mut tab, KeyCode::Enter);

    assert!(typing && tab.prompt().is_none());
    assert_eq!(tab.current_name().as_deref(), Some("web"));
}

#[test]
fn space_folds_a_profile_and_picks_nothing() {
    let mut tab = loaded();

    char_key(&mut tab, ' ');

    assert_eq!(tab.list.visible(), [0, 2, 3, 4]);
}

#[test]
fn a_new_snapshot_keeps_the_cursor_on_the_same_row() {
    let mut tab = loaded();
    press(&mut tab, KeyCode::Down);
    press(&mut tab, KeyCode::Down);

    tab.load(&model());

    assert_eq!(tab.current_name().as_deref(), Some("web"));
}

#[test]
fn drawing_an_empty_tab_does_not_panic() {
    let mut tab = ProfilesTab::new();

    // Assert the public drawing path accepts an empty snapshot.
    render_to_buffer(80, 24, |frame| {
        tab.draw(frame, frame.area(), &Theme::dark())
    });
}

#[test]
fn a_new_profile_cannot_take_an_existing_name_and_keeps_its_picks() {
    let mut tab = loaded();
    char_key(&mut tab, 'n');
    type_in(&mut tab, "api");

    let act = press(&mut tab, KeyCode::Enter);

    assert_eq!(act, None);
    assert!(tab.captures_keys());
}

#[test]
fn saving_with_a_blank_name_from_the_items_returns_to_the_name() {
    let mut tab = loaded();
    char_key(&mut tab, 'n');
    press(&mut tab, KeyCode::Tab);

    let act = char_key(&mut tab, 's');
    type_in(&mut tab, "docs");
    let named = press(&mut tab, KeyCode::Enter);

    assert_eq!(act, None);
    assert!(matches!(named, Some(Act::CreateProfile { .. })));
}
