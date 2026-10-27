//! Doing what the workspace asked for.

use super::Session;
use super::change::{Change, change};
use super::notice::Notice;
use super::selection::Selection;
use crate::app::workspace::Act;
use skillz_harness::model::Scope;

impl Session<'_, '_> {
    /// What the toast should say, if anything.
    pub(in crate::cli) fn perform(&self, act: Act) -> anyhow::Result<Option<Notice>> {
        match &act {
            Act::Quit => Ok(None),
            Act::Change {
                scope,
                install,
                remove,
            } => {
                let picked = Selection {
                    install,
                    remove,
                    only: None,
                };
                change(self, *scope, &Change::Selection(&picked))
            }
            Act::Update { scope, ids } => change(self, *scope, &Change::Update(ids)),
        }
    }

    /// Apply `profile` at `scope`, and remember it as that level's once the change went through.
    pub(in crate::cli) fn apply(
        &self,
        scope: Scope,
        profile: &str,
        what: &Change,
    ) -> anyhow::Result<Option<Notice>> {
        let notice = change(self, scope, what)?;
        if matches!(notice, Some(Notice::Applied(_) | Notice::UpToDate)) {
            self.remember_applied(scope, profile)?;
        }
        Ok(notice)
    }
}
