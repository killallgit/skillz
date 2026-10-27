use ratatui::crossterm::event::KeyCode;

use super::*;
use crate::tui::widgets::margin::MARGIN_X;
use crate::tui::widgets::testing::{ctrl, key, render_to_buffer, row_text};

fn workspace() -> Workspace {
    Workspace::plugins(Level::Project)
}

fn press(workspace: &mut Workspace, code: KeyCode) -> Option<Act> {
    workspace.handle(key(code))
}

#[test]
fn q_and_ctrl_c_quit_and_ctrl_c_quits_even_while_the_filter_is_typed() {
    let mut ws = workspace();
    let mut typing = workspace();
    press(&mut typing, KeyCode::Char('/'));

    let q = press(&mut ws, KeyCode::Char('q'));
    let interrupt = typing.handle(ctrl('c'));

    assert_eq!((q, interrupt), (Some(Act::Quit), Some(Act::Quit)));
}

#[test]
fn while_the_filter_is_typed_q_is_text_not_quit() {
    let mut ws = workspace();
    press(&mut ws, KeyCode::Char('/'));

    let act = press(&mut ws, KeyCode::Char('q'));

    assert_eq!(act, None);
    assert!(ws.list.prompt().is_some());
}

#[test]
fn help_takes_the_keys_until_it_is_closed() {
    let mut ws = workspace();

    press(&mut ws, KeyCode::Char('?'));
    let open = ws.help.is_some();
    let quit_while_open = press(&mut ws, KeyCode::Char('q'));
    press(&mut ws, KeyCode::Esc);

    assert!(open && ws.help.is_none());
    assert_eq!(quit_while_open, None);
}

#[test]
fn a_notice_becomes_a_toast() {
    let mut ws = workspace();

    ws.load(Model {
        notice: Some("done".into()),
        ..Model::default()
    });

    assert_eq!(ws.toasts.len(), 1);
}

#[test]
fn a_terminal_below_the_minimum_draws_only_a_notice() {
    let mut ws = workspace();
    let theme = Theme::dark();

    let buffer = render_to_buffer(MIN_WIDTH - 1, MIN_HEIGHT, |f| ws.draw(f, &theme));

    let used = (0..MIN_HEIGHT).filter(|&y| !row_text(&buffer, y).trim().is_empty());
    assert_eq!(used.count(), 2);
}

#[test]
fn nothing_is_drawn_in_the_margin_columns() {
    let mut ws = workspace();
    let theme = Theme::dark();

    let buffer = render_to_buffer(60, 12, |f| ws.draw(f, &theme));

    let left_edge = (0..12).all(|y| (0..MARGIN_X).all(|x| buffer[(x, y)].symbol() == " "));
    let right_edge = (0..12).all(|y| (60 - MARGIN_X..60).all(|x| buffer[(x, y)].symbol() == " "));
    assert!(left_edge && right_edge);
}

#[test]
fn the_list_is_drawn_bare_with_no_frame() {
    let mut ws = workspace();
    let theme = Theme::dark();

    let buffer = render_to_buffer(60, 12, |f| ws.draw(f, &theme));

    let drawn: String = (0..12).map(|y| row_text(&buffer, y)).collect();
    let borders = ['╭', '╮', '╰', '╯', '│', '─', '├', '┤'];
    assert!(!drawn.contains(borders.as_slice()));
}

#[test]
fn a_finished_sync_toasts_its_notice() {
    let mut ws = workspace();
    let (tx, rx) = std::sync::mpsc::channel();
    ws.watch_syncs(rx);
    ws.load(Model::default());
    tx.send(Synced {
        library: Some(Default::default()),
        notice: Some("library: 2 new".into()),
    })
    .unwrap();
    let theme = Theme::dark();

    render_to_buffer(60, 12, |f| ws.draw(f, &theme));

    assert_eq!(ws.toasts.len(), 1);
}

#[test]
fn a_sync_with_nothing_to_say_toasts_nothing() {
    let mut ws = workspace();
    let (tx, rx) = std::sync::mpsc::channel();
    ws.watch_syncs(rx);
    tx.send(Synced::default()).unwrap();
    let theme = Theme::dark();

    render_to_buffer(60, 12, |f| ws.draw(f, &theme));

    assert_eq!(ws.toasts.len(), 0);
}
