use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use super::*;
use crate::tui::theme::Theme;
use crate::tui::widgets::testing::{key, keys, render_to_buffer};
use ratatui::crossterm::event::KeyCode;

fn model() -> Model {
    let project = |name: &str| ProjectRow {
        path: name.into(),
        profile: None,
        short: name.into(),
        missing: false,
        detail: vec![],
    };
    let profile = |name: &str| ProfileRow {
        name: name.into(),
        detail: vec![],
        broken: false,
    };
    Model {
        places: Default::default(),
        current: None,
        projects: ["a", "b", "c"].into_iter().map(project).collect(),
        profiles: ["x", "y"].into_iter().map(profile).collect(),
        notice: None,
        undo: None,
    }
}

/// A home drawn once at 120x24, so its panes know where they are: projects start at column 1,
/// profiles at column 49, and the first row of each list is screen row 2.
fn drawn() -> Home {
    let mut home = Home::new(model(), Pos::default());
    render_to_buffer(120, 24, |f| home.draw(f, &Theme::dark()));
    home
}

const PROJECTS_X: u16 = 5;
const PROFILES_X: u16 = 52;
const FIRST_ROW: u16 = 2;

fn mouse(kind: MouseEventKind, x: u16, y: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }
}

fn click(home: &mut Home, x: u16, y: u16, at: Instant) -> Option<Act> {
    home.on_mouse(mouse(MouseEventKind::Down(MouseButton::Left), x, y), at)
}

#[test]
fn clicking_a_row_focuses_its_pane_and_selects_it() {
    let mut home = drawn();
    let now = Instant::now();

    let act = click(&mut home, PROFILES_X, FIRST_ROW + 1, now);

    assert_eq!(act, None);
    assert_eq!((home.pos().focus, home.pos().profile), (Focus::Profiles, 1));
}

#[test]
fn clicking_below_the_last_row_focuses_the_pane_but_keeps_the_cursor() {
    let mut home = drawn();
    let now = Instant::now();
    click(&mut home, PROJECTS_X, FIRST_ROW + 1, now);

    click(&mut home, PROFILES_X, FIRST_ROW + 10, now);

    assert_eq!(
        (home.pos().focus, home.pos().project, home.pos().profile),
        (Focus::Profiles, 1, 0)
    );
}

#[test]
fn a_second_click_on_the_same_row_in_time_applies() {
    let mut home = drawn();
    let now = Instant::now();
    click(&mut home, PROJECTS_X, FIRST_ROW + 1, now);
    home.focus_on(Focus::Profiles);
    click(&mut home, PROFILES_X, FIRST_ROW, now);

    let first = click(
        &mut home,
        PROFILES_X,
        FIRST_ROW,
        now + Duration::from_millis(100),
    );

    assert_eq!(
        first,
        Some(Act::Apply {
            project: "b".into(),
            profile: "x".into()
        })
    );
}

#[test]
fn a_slow_second_click_is_just_another_click() {
    let mut home = drawn();
    let now = Instant::now();
    click(&mut home, PROJECTS_X, FIRST_ROW, now);

    let slow = click(
        &mut home,
        PROJECTS_X,
        FIRST_ROW,
        now + Duration::from_secs(2),
    );

    assert_eq!(slow, None);
}

#[test]
fn two_clicks_on_different_rows_are_not_a_double_click() {
    let mut home = drawn();
    let now = Instant::now();
    click(&mut home, PROJECTS_X, FIRST_ROW, now);

    let other = click(&mut home, PROJECTS_X, FIRST_ROW + 1, now);

    assert_eq!(other, None);
}

#[test]
fn the_wheel_scrolls_the_pane_under_the_pointer_without_moving_focus() {
    let mut home = drawn();

    home.on_mouse(
        mouse(MouseEventKind::ScrollDown, PROFILES_X, FIRST_ROW),
        Instant::now(),
    );

    // The profiles cursor moved (to its last row) while the projects pane kept focus.
    assert_eq!((home.pos().focus, home.pos().profile), (Focus::Projects, 1));
}

#[test]
fn the_mouse_does_nothing_while_a_popup_is_open() {
    let mut home = drawn();
    for k in keys("a") {
        home.handle(k);
    }

    let act = click(&mut home, PROFILES_X, FIRST_ROW + 1, Instant::now());
    home.handle(key(KeyCode::Esc));

    assert_eq!((act, home.pos().focus), (None, Focus::Projects));
}

#[test]
fn clicks_outside_every_pane_are_ignored() {
    let mut home = drawn();

    let act = click(&mut home, 5, 0, Instant::now());

    assert_eq!((act, home.pos().focus), (None, Focus::Projects));
}
