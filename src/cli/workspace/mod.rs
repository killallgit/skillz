//! The workspace for one directory: what it and the user have installed, the library and the
//! profiles, with installs, removals and profile applies shown as a plan first, as for `use`.

mod change;
mod library_change;
mod model;
mod notice;
mod perform;
mod plan_view;
mod profiles;
mod selection;
mod updates;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use super::deps::Deps;
use super::scope::resolve_project;
use crate::model::Augmentation;
use crate::profile;
use crate::store;
use crate::tui::workspace::{Act, Shared, Workspace};
use crate::update::check;
use model::snapshot;
use notice::Notice;

/// The project, stores and prompts one session works with.
struct Session<'a, 'd> {
    deps: &'a Deps<'d>,
    profiles: profile::Store,
    store: store::Store,
    project: PathBuf,
}

/// Open the workspace on `path` (default: the current directory). An act that fails shows its
/// error as a toast and the workspace stays open. The terminal is entered once and kept until
/// the workspace closes.
pub fn workspace(deps: &Deps, path: Option<&Path>) -> anyhow::Result<()> {
    let _held = deps.prompter.hold();
    let env = resolve_project(deps, path)?;
    let session = Session {
        deps,
        profiles: profile::Store::in_env(&env),
        store: store::Store::in_env(&env),
        project: env.project,
    };
    let screen: Shared = Arc::new(Mutex::new(Workspace::standard()));
    let (found, results) = mpsc::channel();
    lock(&screen).watch_updates(results);
    let mut notice = None;
    let mut to_check = None;
    loop {
        let model = snapshot(&session, notice.take())?;
        let held: Vec<Augmentation> = [&model.installed[..], &model.global[..]].concat();
        check_in_background(deps, held, to_check.take(), &found);
        lock(&screen).load(model);
        let act = deps.prompter.workspace(&screen)?;
        if act == Act::Quit {
            return Ok(());
        }
        if let Act::Update { ids, .. } = &act {
            lock(&screen).forget_updates(ids);
            to_check = Some(ids.clone());
        }
        notice = session
            .perform(act)
            .unwrap_or_else(|e| Some(Notice::Failed(format!("{e:#}"))));
    }
}

fn lock(screen: &Shared) -> MutexGuard<'_, Workspace> {
    screen.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Look for newer versions of `only` those installed items, project and global alike (all of
/// them on the first call).
fn check_in_background(
    deps: &Deps,
    installed: Vec<Augmentation>,
    only: Option<Vec<String>>,
    found: &Sender<String>,
) {
    let due = installed
        .into_iter()
        .filter(|item| only.as_ref().is_none_or(|ids| ids.contains(&item.id)))
        .collect();
    check::spawn(deps.checker.clone(), due, found.clone());
}
