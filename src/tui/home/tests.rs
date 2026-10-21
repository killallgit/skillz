use super::state::Popup;
use super::view::fit_tail;
use super::*;
use crate::tui::keys::keys;
use crate::tui::widgets::testing::{ctrl, key, render_to_buffer};
use ratatui::crossterm::event::KeyCode;

fn project(label: &str, loaded: bool, missing: bool) -> ProjectRow {
    ProjectRow {
        path: label.into(),
        label: label.into(),
        missing,
        detail: vec![],
        loaded,
    }
}

fn profile(name: &str, broken: bool) -> ProfileRow {
    ProfileRow {
        name: name.into(),
        detail: vec![],
        broken,
    }
}

fn model() -> Model {
    Model {
        projects: vec![project("a", false, false), project("b", true, false)],
        profiles: vec![profile("x", false), profile("y", false)],
        notice: None,
    }
}

fn home() -> Home {
    Home::new(model(), Pos::default())
}

fn press(h: &mut Home, s: &str) -> Option<Act> {
    keys(s).into_iter().filter_map(|k| h.handle(k)).last()
}

#[test]
fn tab_moves_between_panes_and_arrows_stay_inside_the_list() {
    let mut h = home();

    press(&mut h, "jjj");
    // Down stops at the last project.
    assert_eq!(h.pos().project, 1);

    h.handle(key(KeyCode::Tab));
    assert_eq!(h.pos().focus, Focus::Profiles);
    press(&mut h, "jjk");
    // Down twice stops at the last profile, up once returns to the first.
    assert_eq!(h.pos().profile, 0);

    h.handle(key(KeyCode::Left));
    assert_eq!(h.pos().focus, Focus::Projects);
}

#[test]
fn enter_applies_the_selected_profile_to_the_selected_project() {
    let mut h = home();
    press(&mut h, "j");
    h.handle(key(KeyCode::Tab));
    press(&mut h, "j");

    let act = h.handle(key(KeyCode::Enter));

    assert_eq!(
        act,
        Some(Act::Apply {
            project: 1,
            profile: 1
        })
    );
}

#[test]
fn nothing_is_applied_to_a_missing_project_or_with_a_broken_profile() {
    let mut h = home();
    h.model.projects[0].missing = true;
    let missing = h.handle(key(KeyCode::Enter));
    h.model.projects[0].missing = false;
    h.model.profiles[0].broken = true;
    let broken = h.handle(key(KeyCode::Enter));
    let mut empty = Home::new(Model::default(), Pos::default());

    let on_empty = (
        empty.handle(key(KeyCode::Enter)),
        press(&mut empty, "jjkdus"),
    );

    // Each refusal yields no request at all.
    assert_eq!((missing, broken, on_empty), (None, None, (None, None)));
}

#[test]
fn editing_keys_name_the_selected_profile() {
    let mut h = home();
    press(&mut h, "j");
    h.handle(key(KeyCode::Tab));
    press(&mut h, "j");

    let installed = press(&mut h, "i");
    let removal = press(&mut h, "r");
    press(&mut h, "t");
    press(&mut h, "skill o/r a");
    let typed = h.handle(key(KeyCode::Enter));

    // Every request carries the cursor's project and profile.
    assert_eq!(
        installed,
        Some(Act::AddInstalled {
            profile: 1,
            project: 1
        })
    );
    assert_eq!(removal, Some(Act::RemoveEntries { profile: 1 }));
    assert_eq!(
        typed,
        Some(Act::AddTyped {
            profile: 1,
            spec: "skill o/r a".into()
        })
    );
}

#[test]
fn profile_keys_do_nothing_without_profiles() {
    let mut none = Home::new(Model::default(), Pos::default());

    let act = press(&mut none, "irt");

    // No request and no text popup opened.
    assert_eq!(act, None);
    assert!(matches!(none.popup, Popup::None));
}

#[test]
fn unload_needs_a_loaded_profile() {
    let mut h = home();
    let on_unloaded = press(&mut h, "u");
    press(&mut h, "j");

    let on_loaded = press(&mut h, "u");

    assert_eq!(
        (on_unloaded, on_loaded),
        (None, Some(Act::Unload { project: 1 }))
    );
}

#[test]
fn typing_a_name_asks_for_the_matching_action() {
    let mut h = home();
    press(&mut h, "a");
    let typing = press(&mut h, "/tmp/x");
    let added = h.handle(key(KeyCode::Enter));
    press(&mut h, "n");
    press(&mut h, "web");
    h.handle(key(KeyCode::Backspace));
    let created = h.handle(key(KeyCode::Enter));
    press(&mut h, "j");
    press(&mut h, "s");
    press(&mut h, "snap");

    let saved = h.handle(key(KeyCode::Enter));

    assert_eq!(typing, None);
    assert_eq!(added, Some(Act::AddProject("/tmp/x".into())));
    assert_eq!(created, Some(Act::NewProfile("we".into())));
    assert_eq!(
        saved,
        Some(Act::SaveProfile {
            name: "snap".into(),
            project: 1
        })
    );
}

#[test]
fn input_can_be_cancelled_and_empty_input_is_not_submitted() {
    let mut h = home();
    press(&mut h, "n");

    let empty = h.handle(key(KeyCode::Enter));
    press(&mut h, "q");
    h.handle(key(KeyCode::Esc));

    // Empty Enter keeps the popup; after Esc, q quits again.
    assert_eq!(empty, None);
    assert_eq!(press(&mut h, "q"), Some(Act::Quit));
}

#[test]
fn deleting_a_profile_waits_for_a_yes_but_forgetting_a_project_does_not() {
    let mut h = home();
    let forgotten = press(&mut h, "d");
    h.handle(key(KeyCode::Tab));

    let declined = press(&mut h, "dn");
    let confirmed = press(&mut h, "dy");

    assert_eq!(forgotten, Some(Act::ForgetProject(0)));
    assert_eq!(declined, None);
    assert_eq!(confirmed, Some(Act::DeleteProfile(0)));
}

#[test]
fn enter_on_the_delete_dialog_keeps_the_default_no() {
    let mut h = home();
    h.handle(key(KeyCode::Tab));
    press(&mut h, "d");

    let act = h.handle(key(KeyCode::Enter));

    // The dialog starts on No, so a stray Enter deletes nothing.
    assert_eq!(act, None);
    assert!(matches!(h.popup, Popup::None));
}

#[test]
fn quitting_and_cursors_surviving_a_refresh() {
    let mut h = home();
    let esc = h.handle(key(KeyCode::Esc));
    let interrupt = h.handle(ctrl('c'));
    let pos = Pos {
        focus: Focus::Profiles,
        project: 9,
        profile: 1,
    };

    let restored = Home::new(model(), pos).pos();

    assert_eq!((esc, interrupt), (Some(Act::Quit), Some(Act::Quit)));
    // The out-of-range project cursor is pulled back to the last row.
    assert_eq!(
        restored,
        Pos {
            focus: Focus::Profiles,
            project: 1,
            profile: 1
        }
    );
}

#[test]
fn a_notice_is_a_toast_that_the_next_key_dismisses() {
    let mut h = Home::new(
        Model {
            notice: Some("done".into()),
            ..model()
        },
        Pos::default(),
    );
    let shown = h.toasts.len();

    h.handle(key(KeyCode::Char('j')));

    assert_eq!((shown, h.toasts.len()), (1, 0));
}

#[test]
fn long_labels_keep_their_profile_column_and_the_end_of_the_path() {
    let label = "web            ~/Code/a/very/long/project/path";
    assert_eq!(fit_tail(label, 80, 15), label);

    let cut = fit_tail(label, 30, 15);

    assert_eq!(cut.chars().count(), 30);
    assert!(cut.starts_with("web            ") && cut.ends_with("path") && cut.contains('…'));
    assert_eq!(fit_tail(label, 10, 15), label, "too narrow to cut sensibly");
}

#[test]
fn the_focused_pane_has_the_accent_border_and_the_other_does_not() {
    let mut h = home();
    let theme = Theme::dark();

    let buffer = render_to_buffer(80, 12, |f| h.draw(f, &theme));

    // Projects (left) is focused; Profiles starts at column 32 of 80 (40%).
    let focus = theme.focus_border.fg;
    assert_eq!(buffer[(0, 1)].fg, focus.unwrap());
    assert_ne!(buffer[(32, 1)].fg, focus.unwrap());
}

#[test]
fn the_cursor_row_of_the_focused_pane_has_the_selection_background() {
    let mut h = home();
    let theme = Theme::dark();

    let buffer = render_to_buffer(80, 12, |f| h.draw(f, &theme));

    // Inside the frame (x=1,y=2) is the first project row.
    assert_eq!(buffer[(5, 2)].bg, theme.selection.bg.unwrap());
    assert_ne!(buffer[(5, 3)].bg, theme.selection.bg.unwrap());
}

#[test]
fn popups_and_empty_models_draw() {
    let theme = Theme::dark();
    for (model, keys_pressed) in [(Model::default(), ""), (model(), "?"), (model(), "n")] {
        let mut h = Home::new(model, Pos::default());
        press(&mut h, keys_pressed);

        let buffer = render_to_buffer(100, 20, |f| h.draw(f, &theme));

        // Three panes each have a rounded corner; an open popup adds a fourth.
        let corners = (0..20)
            .flat_map(|y| (0..100).map(move |x| (x, y)))
            .filter(|&pos| buffer[pos].symbol() == "╭")
            .count();
        assert_eq!(corners, 3 + usize::from(!keys_pressed.is_empty()));
    }
}
