use std::collections::HashSet;

use crate::action::{self, Action, Report};
use crate::model::Augmentation;
use crate::profile::Entry;
use crate::remove::{self, Outcome, Plan};
use crate::runner::CommandRunner;

use super::switch::Switch;
use super::{Applier, Loaded, Owned, Record};

/// What a switch did.
#[derive(Debug)]
pub struct Done {
    /// `None` when nothing needed removing.
    pub removed: Option<Outcome>,
    pub installed: Report,
    /// Items the new entries added.
    pub added: Vec<Owned>,
}

/// The entries installed so far, and the error that stopped the rest.
struct Installs {
    loaded: Vec<Loaded>,
    report: Report,
    failure: Option<anyhow::Error>,
}

impl Applier<'_> {
    /// Remove the dropped entries' items, install the new entries one at a time, and record
    /// what each added. A step that fails still records what landed before it, then returns the
    /// error.
    pub fn run(
        &self,
        switch: &Switch,
        runner: &dyn CommandRunner,
        on_step: &mut dyn FnMut(&Action),
    ) -> anyhow::Result<Done> {
        let (removed, leftover) = self.remove_dropped(switch, runner, on_step)?;
        let installs = self.install_all(&switch.installs, runner, on_step);
        let added: Vec<Owned> = installs
            .loaded
            .iter()
            .flat_map(|l| l.items.clone())
            .collect();

        let mut entries = switch.keep.clone();
        entries.extend(leftover);
        entries.extend(installs.loaded);
        let record = next_record(switch, entries, installs.failure.is_some());
        self.ledger.put(&self.ledger_key(), record)?;

        match installs.failure {
            Some(e) => Err(e),
            None => Ok(Done {
                removed,
                installed: installs.report,
                added,
            }),
        }
    }

    /// Run the removals; the dropped entries whose items are still installed stay recorded.
    fn remove_dropped(
        &self,
        switch: &Switch,
        runner: &dyn CommandRunner,
        on_step: &mut dyn FnMut(&Action),
    ) -> anyhow::Result<(Option<Outcome>, Vec<Loaded>)> {
        if switch.removals.is_empty() {
            return Ok((None, vec![]));
        }
        let refs: Vec<&Augmentation> = switch.removals.iter().collect();
        let outcome = remove::run(
            &Plan::new(&refs, &[]),
            self.adapters,
            self.env,
            &[self.scope],
            runner,
            on_step,
        )?;
        let leftover = still_installed(&switch.drop, &outcome);
        Ok((Some(outcome), leftover))
    }

    fn install_all(
        &self,
        installs: &[(Entry, Vec<Action>)],
        runner: &dyn CommandRunner,
        on_step: &mut dyn FnMut(&Action),
    ) -> Installs {
        let mut done = Installs {
            loaded: vec![],
            report: Report::default(),
            failure: None,
        };
        for (entry, steps) in installs {
            let (loaded, ran) = self.install_entry(entry, steps, runner, on_step);
            done.loaded.extend(loaded);
            match ran {
                Ok(report) => {
                    done.report.ran.extend(report.ran);
                    done.report.skipped.extend(report.skipped);
                }
                Err(e) => {
                    done.failure = Some(e);
                    break;
                }
            }
        }
        done
    }

    /// Run one entry's steps and record what appeared. A failed run that still installed
    /// something is recorded too, so it can be unloaded.
    fn install_entry(
        &self,
        entry: &Entry,
        steps: &[Action],
        runner: &dyn CommandRunner,
        on_step: &mut dyn FnMut(&Action),
    ) -> (Option<Loaded>, anyhow::Result<Report>) {
        let before: HashSet<String> = self.installed().into_iter().map(|i| i.id).collect();
        let ran = action::execute_with(steps, runner, on_step);
        let items: Vec<Owned> = self
            .installed()
            .iter()
            .filter(|i| !before.contains(&i.id))
            .map(Owned::of)
            .collect();
        let recorded = (ran.is_ok() || !items.is_empty()).then(|| Loaded {
            entry: entry.clone(),
            items,
        });
        (recorded, ran)
    }
}

/// The dropped entries with only the items that survived the removal.
fn still_installed(dropped: &[Loaded], outcome: &Outcome) -> Vec<Loaded> {
    dropped
        .iter()
        .filter_map(|l| {
            let items: Vec<Owned> = l
                .items
                .iter()
                .filter(|o| outcome.still_installed.contains(&o.name))
                .cloned()
                .collect();
            (!items.is_empty()).then(|| Loaded {
                entry: l.entry.clone(),
                items,
            })
        })
        .collect()
}

/// The ledger entry after a switch: none when nothing is loaded any more, or when the first
/// install failed before anything landed.
fn next_record(switch: &Switch, entries: Vec<Loaded>, failed: bool) -> Option<Record> {
    let nothing = entries.is_empty() && (switch.to.is_none() || failed);
    let profile = switch.to.clone().or_else(|| switch.from.clone());
    match (profile, nothing) {
        (Some(profile), false) => Some(Record { profile, entries }),
        _ => None,
    }
}
