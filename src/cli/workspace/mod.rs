//! The workspace for one directory: what it and the user have installed, the library and the
//! profiles, with installs, removals and profile applies shown as a plan first, as for `use`.

mod model;
mod sync;

use std::path::Path;
use std::sync::mpsc;
use std::sync::{MutexGuard, PoisonError};

use super::deps::Deps;
use super::scope::resolve_project;
use super::session::{Notice, Session};
use crate::harness::update::check;
use crate::workspace::{Act, Screen, SharedScreen};
use model::snapshot;

/// Open the workspace on `path` (default: the current directory). An act that fails shows its
/// error as a toast and the workspace stays open. The terminal is entered once and kept until
/// the workspace closes.
pub fn workspace(deps: &Deps, path: Option<&Path>) -> anyhow::Result<()> {
    let _held = deps.prompter.hold();
    let env = resolve_project(deps, path)?;
    let (synced, syncs) = mpsc::channel();
    sync::start(deps, &env, synced);
    let session = Session::open(deps, &env);
    let screen = deps.prompter.screen()?;
    let (found, results) = mpsc::channel();
    lock(&screen).watch_updates(results);
    lock(&screen).watch_syncs(syncs);
    let mut notice = None;
    let mut checked = std::collections::HashSet::new();
    loop {
        let model = snapshot(&session, notice.take())?;
        let due = model
            .installed
            .iter()
            .chain(&model.global)
            .filter(|item| checked.insert(item.id.clone()))
            .cloned()
            .collect::<Vec<_>>();
        if !due.is_empty() {
            check::spawn(deps.checker.clone(), due, found.clone());
        }
        lock(&screen).load(model);
        let act = deps.prompter.workspace()?;
        if act == Act::Quit {
            return Ok(());
        }
        if let Act::Update { ids, .. } = &act {
            lock(&screen).forget_updates(ids);
            for id in ids {
                checked.remove(id);
            }
        }
        notice = session
            .perform(act)
            .unwrap_or_else(|e| Some(Notice::Failed(format!("{e:#}"))));
    }
}

fn lock(screen: &SharedScreen) -> MutexGuard<'_, dyn Screen + 'static> {
    screen.lock().unwrap_or_else(PoisonError::into_inner)
}
