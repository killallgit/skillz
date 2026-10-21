use super::state::Popup;
use super::*;
use crate::tui::theme::Theme;
use crate::tui::widgets::testing::{ctrl, key, keys, render_to_buffer};
use ratatui::crossterm::event::KeyCode;

fn project(label: &str, loaded: bool, missing: bool) -> ProjectRow {
    ProjectRow {
        path: label.into(),
        profile: loaded.then(|| "web".to_string()),
        short: label.into(),
        missing,
        detail: vec![],
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
        places: Default::default(),
        current: None,
        projects: vec![project("a", false, false), project("b", true, false)],
        profiles: vec![profile("x", false), profile("y", false)],
        notice: None,
        undo: None,
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
            project: "b".into(),
            profile: "y".into()
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
            profile: "y".into(),
            project: "b".into()
        })
    );
    assert_eq!(
        removal,
        Some(Act::RemoveEntries {
            profile: "y".into()
        })
    );
    assert_eq!(
        typed,
        Some(Act::AddTyped {
            profile: "y".into(),
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
        (
            None,
            Some(Act::Unload {
                project: "b".into()
            })
        )
    );
}

#[test]
fn typing_a_name_asks_for_the_matching_action() {
    let mut h = home();
    let typing = press(&mut h, "a");
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
    assert!(matches!(added, Some(Act::AddProject(_))));
    assert_eq!(created, Some(Act::NewProfile("we".into())));
    assert_eq!(
        saved,
        Some(Act::SaveProfile {
            name: "snap".into(),
            project: "b".into()
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

    assert_eq!(forgotten, Some(Act::ForgetProject("a".into())));
    assert_eq!(declined, None);
    assert_eq!(confirmed, Some(Act::DeleteProfile("x".into())));
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
            places: Default::default(),
            notice: Some("done".into()),
            undo: None,
            ..model()
        },
        Pos::default(),
    );
    let shown = h.toasts.len();

    h.handle(key(KeyCode::Char('j')));

    assert_eq!((shown, h.toasts.len()), (1, 0));
}

#[test]
fn the_focused_pane_has_the_accent_border_and_the_other_does_not() {
    let mut h = home();
    let theme = Theme::dark();

    let buffer = render_to_buffer(120, 12, |f| h.draw(f, &theme));

    // Projects (left) is focused; Profiles starts at column 48 of 120 (40%).
    let focus = theme.focus_border.fg;
    assert_eq!(buffer[(0, 1)].fg, focus.unwrap());
    assert_ne!(buffer[(48, 1)].fg, focus.unwrap());
}

#[test]
fn the_cursor_row_of_the_focused_pane_has_the_selection_background() {
    let mut h = home();
    let theme = Theme::dark();

    let buffer = render_to_buffer(120, 12, |f| h.draw(f, &theme));

    // Inside the frame (x=1,y=2) is the first project row.
    assert_eq!(buffer[(5, 2)].bg, theme.selection.bg.unwrap());
    assert_ne!(buffer[(5, 3)].bg, theme.selection.bg.unwrap());
}

fn corners(buffer: &ratatui::buffer::Buffer) -> usize {
    let area = buffer.area;
    (0..area.height)
        .flat_map(|y| (0..area.width).map(move |x| (x, y)))
        .filter(|&pos| buffer[pos].symbol() == "╭")
        .count()
}

#[test]
fn panes_follow_the_terminal_width() {
    let theme = Theme::dark();
    let sizes = [(120, 20), (80, 20), (50, 20), (30, 8)];

    let drawn: Vec<usize> = sizes
        .iter()
        .map(|&(w, h)| {
            let mut screen = home();
            corners(&render_to_buffer(w, h, |f| screen.draw(f, &theme)))
        })
        .collect();

    // Three panes, three panes (two columns), one pane behind tabs, nothing when too small.
    assert_eq!(drawn, [3, 3, 1, 0]);
}

#[test]
fn popups_add_a_frame_and_an_empty_workspace_shows_one_welcome_frame() {
    let theme = Theme::dark();
    let cases = [(Model::default(), "", 1), (model(), "n", 4)];
    for (model, pressed, expected) in cases {
        let mut h = Home::new(model, Pos::default());
        press(&mut h, pressed);

        let buffer = render_to_buffer(120, 24, |f| h.draw(f, &theme));

        assert_eq!(corners(&buffer), expected);
    }
}

#[test]
fn an_empty_pane_hints_while_the_other_is_filled() {
    let theme = Theme::dark();
    let mut only_profiles = Home::new(
        Model {
            places: Default::default(),
            profiles: vec![profile("x", false)],
            ..Model::default()
        },
        Pos::default(),
    );

    let buffer = render_to_buffer(120, 20, |f| only_profiles.draw(f, &theme));

    // Three panes, not the welcome; the empty Projects pane has a centred muted hint.
    assert_eq!(corners(&buffer), 3);
    let muted_hint = (0..20)
        .flat_map(|y| (1..47).map(move |x| (x, y)))
        .any(|pos| buffer[pos].fg == theme.muted.fg.unwrap() && buffer[pos].symbol() != " ");
    assert!(muted_hint);
}

#[test]
fn slash_filters_the_focused_pane_and_requests_name_model_rows() {
    let mut h = home();
    press(&mut h, "/b");

    let enter = h.handle(key(KeyCode::Enter));
    h.handle(key(KeyCode::Tab));
    let applied = h.handle(key(KeyCode::Enter));

    // Only "b" remains; Enter closed the prompt, and the request carries b's model row (1).
    assert_eq!(enter, None);
    assert_eq!(h.projects.visible(), [1]);
    assert_eq!(
        applied,
        Some(Act::Apply {
            project: "b".into(),
            profile: "x".into()
        })
    );
}

#[test]
fn esc_closes_the_topmost_layer_before_leaving() {
    let mut h = home();
    press(&mut h, "/b");
    h.handle(key(KeyCode::Enter));

    let clears = h.handle(key(KeyCode::Esc));
    let filtered_after = h.projects.is_filtered();
    let leaves = h.handle(key(KeyCode::Esc));

    // The first Esc only drops the filter; the second leaves the screen.
    assert_eq!(
        (clears, filtered_after, leaves),
        (None, false, Some(Act::Quit))
    );
}

#[test]
fn esc_in_the_filter_prompt_cancels_it_and_q_is_only_text_there() {
    let mut h = home();
    press(&mut h, "/q");
    let still_open = matches!(h.popup, Popup::Filter);

    h.handle(key(KeyCode::Esc));

    assert!(still_open);
    assert!(matches!(h.popup, Popup::None) && !h.projects.is_filtered());
}

#[test]
fn q_does_nothing_inside_a_popup_but_ctrl_c_always_quits() {
    let mut h = home();
    press(&mut h, "?");

    let q = h.handle(key(KeyCode::Char('q')));
    let interrupt = h.handle(ctrl('c'));
    press(&mut h, "n");
    let in_input = h.handle(ctrl('c'));

    assert_eq!(
        (q, interrupt, in_input),
        (None, Some(Act::Quit), Some(Act::Quit))
    );
}

#[test]
fn the_help_overlay_closes_on_esc_and_question_mark() {
    let mut h = home();
    press(&mut h, "?");
    let open = matches!(h.popup, Popup::Help(_));

    h.handle(key(KeyCode::Esc));
    let closed = matches!(h.popup, Popup::None);
    press(&mut h, "??");

    assert!(open && closed);
    assert!(matches!(h.popup, Popup::None));
}

#[test]
fn top_and_bottom_keys_jump_the_cursor() {
    let mut h = home();

    press(&mut h, "G");
    let bottom = h.pos().project;
    press(&mut h, "g");

    assert_eq!((bottom, h.pos().project), (1, 0));
}

#[test]
fn single_pane_mode_switches_panes_with_number_keys() {
    let theme = Theme::dark();
    let mut h = home();
    render_to_buffer(50, 20, |f| h.draw(f, &theme));

    press(&mut h, "2");
    let on_profiles = h.pos().focus;
    press(&mut h, "j");
    let moved = h.pos().profile;
    press(&mut h, "3");
    press(&mut h, "k");

    // Tab 2 is Profiles and moves its cursor; on the Detail tab the cursor keys are inert.
    assert_eq!(
        (on_profiles, moved, h.pos().profile),
        (Focus::Profiles, 1, 1)
    );
}

#[test]
fn the_header_colours_a_missing_project_with_the_danger_style() {
    let theme = Theme::dark();
    let mut h = Home::new(
        Model {
            places: Default::default(),
            projects: vec![project("a", false, true)],
            ..Model::default()
        },
        Pos::default(),
    );

    let buffer = render_to_buffer(120, 20, |f| h.draw(f, &theme));

    let danger_in_header = (0..120).any(|x| buffer[(x, 0)].fg == theme.danger.fg.unwrap());
    assert!(danger_in_header);
}

#[test]
fn the_undo_key_replays_the_request_the_model_carried() {
    let undo = Act::Unload {
        project: "b".into(),
    };
    let mut with = Home::new(
        Model {
            places: Default::default(),
            undo: Some(undo.clone()),
            ..model()
        },
        Pos::default(),
    );
    let mut without = home();

    let (asked, idle) = (press(&mut with, "U"), press(&mut without, "U"));

    // With an undo on offer U asks for it; without one it does nothing.
    assert_eq!((asked, idle), (Some(undo), None));
}

#[test]
fn profiles_in_use_are_counted_by_the_projects_that_have_them_loaded() {
    let h = home();

    let counts = (h.projects_using("web"), h.projects_using("x"));

    // Project b has `web` loaded; nothing has `x`.
    assert_eq!(counts, (1, 0));
}
