use std::collections::HashSet;

use crate::action::{self, Action, Report};
use crate::profile::Entry;
use crate::remove::{self, Outcome, Plan, Removal};
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

impl Applier<'_> {
    /// Remove the dropped entries' items, install the new entries one at a time, and record
    /// what each added. The ledger is rewritten after the removals and after every entry, so a
    /// crash or a failing step loses nothing that already landed.
    pub fn run(
        &self,
        switch: &Switch,
        runner: &dyn CommandRunner,
        on_step: &mut dyn FnMut(&Action),
    ) -> anyhow::Result<Done> {
        let (removed, leftover) = self.remove_dropped(switch, runner, on_step)?;
        let mut entries = switch.keep.clone();
        entries.extend(leftover);
        self.save(switch, &entries, false)?;

        let mut installed = Report::default();
        let mut added = Vec::new();
        for (entry, steps) in &switch.installs {
            let (loaded, ran) = self.install_entry(entry, steps, runner, on_step);
            if let Some(loaded) = loaded {
                added.extend(loaded.items.clone());
                entries.push(loaded);
                self.save(switch, &entries, false)?;
            }
            let report = ran?;
            installed.ran.extend(report.ran);
            installed.skipped.extend(report.skipped);
        }
        self.save(switch, &entries, true)?;
        Ok(Done {
            removed,
            installed,
            added,
        })
    }

    fn save(&self, switch: &Switch, entries: &[Loaded], complete: bool) -> anyhow::Result<()> {
        let record = next_record(switch, entries.to_vec(), complete);
        self.ledger.put(&self.ledger_key(), record)
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
        let removal = Removal {
            adapters: self.adapters,
            env: self.env,
            scopes: &[self.scope],
            runner,
        };
        let outcome = remove::run(&Plan::from_owned(&switch.removals), &removal, on_step)?;
        let leftover = still_installed(&switch.drop, &outcome);
        Ok((Some(outcome), leftover))
    }

    /// Run one entry's steps and record what appeared. A failed run that still installed
    /// something is recorded too, so it can be unloaded. Nothing runs when the inventory
    /// cannot be read, since what appeared could not be told from what was already there.
    fn install_entry(
        &self,
        entry: &Entry,
        steps: &[Action],
        runner: &dyn CommandRunner,
        on_step: &mut dyn FnMut(&Action),
    ) -> (Option<Loaded>, anyhow::Result<Report>) {
        let before: HashSet<String> = match self.installed() {
            Ok(items) => items.into_iter().map(|i| i.id).collect(),
            Err(e) => return (None, Err(e)),
        };
        let ran = action::execute_with(steps, runner, on_step);
        let items: Vec<Owned> = match self.installed() {
            Ok(now) => now
                .iter()
                .filter(|i| !before.contains(&i.id))
                .map(Owned::of)
                .collect(),
            Err(e) => return (None, Err(ran.err().unwrap_or(e))),
        };
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
                .filter(|o| outcome.still_installed.iter().any(|i| i.id == o.id))
                .cloned()
                .collect();
            (!items.is_empty()).then(|| Loaded {
                entry: l.entry.clone(),
                items,
            })
        })
        .collect()
}

/// The ledger entry after a switch step: none when nothing is loaded any more, or when the
/// installs have not landed anything yet.
fn next_record(switch: &Switch, entries: Vec<Loaded>, complete: bool) -> Option<Record> {
    let nothing = entries.is_empty() && (switch.to.is_none() || !complete);
    let profile = switch.to.clone().or_else(|| switch.from.clone());
    match (profile, nothing) {
        (Some(profile), false) => Some(Record { profile, entries }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Report;
    use crate::model::{Augmentation, Installer, Kind, Scope};

    fn augmentation(id: &str, name: &str) -> Augmentation {
        Augmentation {
            hooks: vec![],
            id: id.into(),
            name: name.into(),
            kind: Kind::Skill,
            agents: vec![],
            scope: Scope::Global,
            group: "g".into(),
            source: None,
            installer: Installer::Local,
            children: vec![],
            removal: vec![],
        }
    }

    #[test]
    fn a_same_named_item_elsewhere_does_not_keep_the_owned_one_recorded() {
        let owned = Owned {
            id: "skill:a".into(),
            name: "tdd".into(),
            kind: Kind::Skill,
        };
        let dropped = [Loaded {
            entry: Entry::Skill {
                source: "o/r".into(),
                names: vec!["tdd".into()],
                agents: vec![],
            },
            items: vec![owned],
        }];
        let outcome = Outcome {
            report: Report::default(),
            still_installed: vec![augmentation("skill:b", "tdd")],
        };

        let left = still_installed(&dropped, &outcome);

        // only the other, hand-installed "tdd" remains; the owned one is gone
        assert!(left.is_empty());
    }
}
