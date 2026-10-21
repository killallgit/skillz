//! What the workspace and the plain commands share for changing things: one project's stores
//! and the changes to the project, library and profiles, each with the plan reviewed first.

mod change;
mod library_change;
mod notice;
mod perform;
mod plan_view;
mod profiles;
mod selection;
mod updates;

use std::path::PathBuf;

use super::deps::Deps;
use crate::env::Env;
use crate::{profile, store};

pub(super) use change::{Change, change};
pub(super) use notice::Notice;
pub(super) use selection::Selection;

/// The project, stores and prompts one session works with.
pub(super) struct Session<'a, 'd> {
    deps: &'a Deps<'d>,
    profiles: profile::Store,
    store: store::Store,
    project: PathBuf,
}

impl<'a, 'd> Session<'a, 'd> {
    pub(super) fn open(deps: &'a Deps<'d>, env: &Env) -> Self {
        Session {
            deps,
            profiles: profile::Store::in_env(env),
            store: store::Store::in_env(env),
            project: env.project.clone(),
        }
    }

    pub(super) fn deps(&self) -> &Deps<'d> {
        self.deps
    }

    pub(super) fn project(&self) -> &std::path::Path {
        &self.project
    }

    pub(super) fn profiles(&self) -> &profile::Store {
        &self.profiles
    }

    pub(super) fn library(&self) -> &store::Store {
        &self.store
    }
}
