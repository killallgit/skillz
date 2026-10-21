use std::sync::{Arc, Mutex};

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::*;
use crate::review::{Phase, Target};
use crate::tui::widgets::testing::{ctrl, key, render_to_buffer, row_text};

/// What a fake tab saw: the keys it was given and how many snapshots it loaded.
#[derive(Default)]
struct Seen {
    keys: Vec<KeyCode>,
    loads: usize,
    notes: usize,
    ticks: usize,
}

struct FakeTab {
    title: &'static str,
    captures: bool,
    seen: Arc<Mutex<Seen>>,
}

impl Tab for FakeTab {
    fn title(&self) -> &'static str {
        self.title
    }

    fn load(&mut self, _model: &Model) {
        self.seen.lock().unwrap().loads += 1;
    }

    fn note(&mut self, _event: &ItemEvent) {
        self.seen.lock().unwrap().notes += 1;
    }

    fn tick(&mut self) {
        self.seen.lock().unwrap().ticks += 1;
    }

    fn handle(&mut self, key: KeyEvent) -> Option<Act> {
        self.seen.lock().unwrap().keys.push(key.code);
        None
    }

    fn draw(&mut self, _frame: &mut Frame, _area: Rect, _theme: &Theme) {}

    fn captures_keys(&self) -> bool {
        self.captures
    }
}

fn workspace(captures: bool) -> (Workspace, Vec<Arc<Mutex<Seen>>>) {
    let seen: Vec<_> = (0..3).map(|_| Arc::default()).collect();
    let tabs = ["one", "two", "three"]
        .into_iter()
        .zip(&seen)
        .map(|(title, seen)| -> Box<dyn Tab> {
            Box::new(FakeTab {
                title,
                captures,
                seen: Arc::clone(seen),
            })
        })
        .collect();
    (Workspace::new(tabs), seen)
}

fn press(workspace: &mut Workspace, code: KeyCode) -> Option<Act> {
    workspace.handle(key(code))
}

#[test]
fn tab_and_shift_tab_step_through_the_tabs_and_digits_jump() {
    let (mut ws, _) = workspace(false);

    press(&mut ws, KeyCode::Tab);
    let after_tab = ws.bar.selected();
    press(&mut ws, KeyCode::BackTab);
    press(&mut ws, KeyCode::BackTab);
    let wrapped = ws.bar.selected();
    press(&mut ws, KeyCode::Char('2'));

    assert_eq!((after_tab, wrapped, ws.bar.selected()), (1, 2, 1));
}

#[test]
fn keys_the_workspace_does_not_use_go_to_the_selected_tab_only() {
    let (mut ws, seen) = workspace(false);
    press(&mut ws, KeyCode::Tab);

    press(&mut ws, KeyCode::Char('x'));

    let given: Vec<usize> = seen.iter().map(|s| s.lock().unwrap().keys.len()).collect();
    assert_eq!(given, [0, 1, 0]);
}

#[test]
fn q_and_ctrl_c_quit_and_ctrl_c_quits_even_while_a_tab_captures_keys() {
    let (mut ws, _) = workspace(false);
    let (mut typing, _) = workspace(true);

    let q = press(&mut ws, KeyCode::Char('q'));
    let interrupt = typing.handle(ctrl('c'));

    assert_eq!((q, interrupt), (Some(Act::Quit), Some(Act::Quit)));
}

#[test]
fn a_capturing_tab_gets_tab_digits_and_q_instead_of_the_workspace() {
    let (mut ws, seen) = workspace(true);

    let acts = [KeyCode::Tab, KeyCode::Char('2'), KeyCode::Char('q')].map(|c| press(&mut ws, c));

    assert_eq!(acts, [None, None, None]);
    assert_eq!(seen[0].lock().unwrap().keys.len(), 3);
    assert_eq!(ws.bar.selected(), 0);
}

#[test]
fn help_takes_the_keys_until_it_is_closed() {
    let (mut ws, seen) = workspace(false);

    press(&mut ws, KeyCode::Char('?'));
    let open = ws.help.is_some();
    press(&mut ws, KeyCode::Char('x'));
    press(&mut ws, KeyCode::Esc);

    assert!(open && ws.help.is_none());
    assert!(seen[0].lock().unwrap().keys.is_empty());
}

#[test]
fn every_tab_gets_each_snapshot_and_a_notice_becomes_a_toast() {
    let (mut ws, seen) = workspace(false);

    ws.load(Model {
        notice: Some("done".into()),
        ..Model::default()
    });

    assert_eq!(
        seen.iter().map(|s| s.lock().unwrap().loads).sum::<usize>(),
        3
    );
    assert_eq!(ws.toasts.len(), 1);
}

#[test]
fn a_terminal_below_the_minimum_draws_only_a_notice() {
    let (mut ws, _) = workspace(false);
    let theme = Theme::dark();

    let buffer = render_to_buffer(MIN_WIDTH - 1, MIN_HEIGHT, |f| ws.draw(f, &theme));

    let used = (0..MIN_HEIGHT).filter(|&y| !row_text(&buffer, y).trim().is_empty());
    assert_eq!(used.count(), 2);
}

#[test]
fn the_tab_bar_names_every_tab_on_the_first_row() {
    let (mut ws, _) = workspace(false);
    let theme = Theme::dark();

    let buffer = render_to_buffer(60, 12, |f| ws.draw(f, &theme));

    let bar = row_text(&buffer, 0);
    assert!(["one", "two", "three"].iter().all(|t| bar.contains(t)));
}

#[test]
fn item_events_and_ticks_reach_every_tab_not_just_the_shown_one() {
    let (mut ws, seen) = workspace(false);
    let event = ItemEvent {
        target: Target::Remove("x".into()),
        label: "x".into(),
        phase: Phase::Started,
    };

    ws.note(&event);
    ws.tick();
    ws.tick();

    let counts: Vec<(usize, usize)> = seen
        .iter()
        .map(|s| {
            let seen = s.lock().unwrap();
            (seen.notes, seen.ticks)
        })
        .collect();
    assert_eq!(counts, [(1, 2); 3]);
}

#[test]
fn the_standard_workspace_has_four_tabs_and_digit_four_reaches_the_last() {
    let mut ws = Workspace::standard();

    let tabs = ws.tabs.len();
    press(&mut ws, KeyCode::Char('4'));

    assert_eq!((tabs, ws.bar.selected()), (4, 3));
}

#[test]
fn a_finished_sync_reloads_every_tab_and_toasts_its_notice() {
    let (mut ws, seen) = workspace(false);
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

    // Assert each tab loaded twice (the snapshot, then the sync) and one toast is up.
    assert!(seen.iter().all(|s| s.lock().unwrap().loads == 2));
    assert_eq!(ws.toasts.len(), 1);
}

#[test]
fn a_sync_with_nothing_to_say_toasts_nothing() {
    let (mut ws, _) = workspace(false);
    let (tx, rx) = std::sync::mpsc::channel();
    ws.watch_syncs(rx);
    tx.send(Synced::default()).unwrap();
    let theme = Theme::dark();

    render_to_buffer(60, 12, |f| ws.draw(f, &theme));

    assert_eq!(ws.toasts.len(), 0);
}
