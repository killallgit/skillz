//! Installing or removing the highlighted row at once, and asking first when an install would
//! duplicate something already installed globally.

use ratatui::crossterm::event::KeyEvent;

use super::rows::{Row, State};
use super::{InstalledTab, Popup};
use crate::model::Scope;
use crate::profile::Entry;
use crate::tui::widgets::confirm::{ConfirmDialog, Outcome as Answer};
use crate::tui::workspace::act::Act;

/// A change waiting on the answer to the global-duplicate question.
#[derive(Debug, Clone)]
pub(super) struct Proposal {
    scope: Scope,
    install: Vec<Entry>,
    remove: Vec<String>,
    /// The part of `install` that is installed globally already.
    held: Vec<Entry>,
}

impl Proposal {
    fn everything(&self) -> Act {
        Act::Change {
            scope: self.scope,
            install: self.install.clone(),
            remove: self.remove.clone(),
        }
    }

    fn without_global(&self) -> Option<Act> {
        let install: Vec<Entry> = self
            .install
            .iter()
            .filter(|entry| !self.held.contains(entry))
            .cloned()
            .collect();
        let anything = !install.is_empty() || !self.remove.is_empty();
        anything.then(|| Act::Change {
            scope: self.scope,
            install,
            remove: self.remove.clone(),
        })
    }
}

impl InstalledTab {
    /// Install the highlighted item into the project; anything else is left alone.
    pub(super) fn install_current(&mut self) -> Option<Act> {
        match self.current_item()?.state.clone() {
            State::Available(entry) => self.propose(vec![entry], vec![]),
            State::Installed(_) => None,
        }
    }

    /// Remove the highlighted installed item; library rows are left alone.
    pub(super) fn remove_current(&mut self) -> Option<Act> {
        match &self.current_item()?.state {
            State::Installed(id) => {
                let remove = vec![id.clone()];
                self.propose(vec![], remove)
            }
            State::Available(_) => None,
        }
    }

    /// The change as an act, unless an install of something installed globally must be
    /// confirmed first.
    pub(super) fn propose(&mut self, install: Vec<Entry>, remove: Vec<String>) -> Option<Act> {
        let held: Vec<Entry> = install
            .iter()
            .filter(|entry| self.is_global(entry))
            .cloned()
            .collect();
        let proposal = Proposal {
            scope: self.scope,
            install,
            remove,
            held,
        };
        if proposal.held.is_empty() {
            return Some(proposal.everything());
        }
        let dialog = ConfirmDialog::new("Installed globally", self.global_question(&proposal));
        self.popup = Popup::AlsoGlobal(dialog, proposal);
        None
    }

    pub(super) fn handle_also_global(
        &mut self,
        mut dialog: ConfirmDialog,
        proposal: Proposal,
        key: KeyEvent,
    ) -> Option<Act> {
        match dialog.handle(key) {
            Answer::Pending => self.popup = Popup::AlsoGlobal(dialog, proposal),
            Answer::Yes => return Some(proposal.everything()),
            Answer::No => return proposal.without_global(),
        }
        None
    }

    fn is_global(&self, entry: &Entry) -> bool {
        self.rows.iter().any(|row| {
            matches!(row, Row::Item(item)
                if item.global && item.state == State::Available(entry.clone()))
        })
    }

    fn global_question(&self, proposal: &Proposal) -> String {
        let names: Vec<&str> = self
            .rows
            .iter()
            .filter_map(|row| match row {
                Row::Item(item) if item.global => Some(item),
                _ => None,
            })
            .filter(|item| matches!(&item.state, State::Available(e) if proposal.held.contains(e)))
            .map(|item| item.label.as_str())
            .collect();
        format!(
            "Already installed globally: {}. Also install into this project?",
            names.join(", ")
        )
    }
}
