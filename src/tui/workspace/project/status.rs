//! Where each row's change stands while it runs: working, done or failed. Held only until the
//! next snapshot or key, so a finished change is never shown for longer than the screen it
//! belongs to.

use super::rows::State;
use crate::profile::same_thing;
use crate::review::{ItemEvent, Phase, Target};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Status {
    Working,
    Done,
    Failed,
}

#[derive(Debug, Clone, Default)]
pub(super) struct Statuses {
    targets: Vec<(Target, Status)>,
    ticks: usize,
}

impl Statuses {
    pub fn note(&mut self, event: &ItemEvent) {
        let status = match event.phase {
            Phase::Started => Status::Working,
            Phase::Succeeded => Status::Done,
            Phase::Failed(_) => Status::Failed,
        };
        match self.targets.iter_mut().find(|(t, _)| *t == event.target) {
            Some(known) => known.1 = status,
            None => self.targets.push((event.target.clone(), status)),
        }
        self.ticks += 1;
    }

    pub fn tick(&mut self) {
        self.ticks += 1;
    }

    pub fn clear(&mut self) {
        self.targets.clear();
    }

    /// How many times the spinner has been asked to move on.
    pub fn ticks(&self) -> usize {
        self.ticks
    }

    /// The status of the row in `state`, when a change is working on it.
    pub fn of(&self, state: &State) -> Option<Status> {
        self.targets
            .iter()
            .find(|(target, _)| concerns(target, state))
            .map(|(_, status)| *status)
    }
}

fn concerns(target: &Target, state: &State) -> bool {
    match (target, state) {
        (Target::Install(entry), State::Available(row)) => same_thing(entry, row),
        (Target::Remove(id) | Target::Update(id), State::Installed(row)) => id == row,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Entry;

    fn skill(name: &str) -> Entry {
        Entry::Skill {
            source: "o/r".into(),
            names: vec![name.into()],
            agents: vec![],
        }
    }

    fn event(target: Target, phase: Phase) -> ItemEvent {
        ItemEvent {
            target,
            label: "x".into(),
            phase,
        }
    }

    #[test]
    fn a_row_is_working_from_the_start_and_then_done_or_failed() {
        let mut statuses = Statuses::default();
        let target = Target::Install(skill("a"));
        let row = State::Available(skill("a"));

        statuses.note(&event(target.clone(), Phase::Started));
        let working = statuses.of(&row);
        statuses.note(&event(target.clone(), Phase::Succeeded));
        let done = statuses.of(&row);
        statuses.note(&event(target, Phase::Failed("no".into())));
        let failed = statuses.of(&row);

        let seen = [working, done, failed];
        assert_eq!(
            seen,
            [
                Some(Status::Working),
                Some(Status::Done),
                Some(Status::Failed)
            ]
        );
    }

    #[test]
    fn an_event_marks_only_the_rows_it_names() {
        let mut statuses = Statuses::default();
        statuses.note(&event(Target::Remove("skill:a".into()), Phase::Started));

        let named = statuses.of(&State::Installed("skill:a".into()));
        let other = statuses.of(&State::Installed("skill:b".into()));
        let library = statuses.of(&State::Available(skill("a")));

        assert_eq!((named, other, library), (Some(Status::Working), None, None));
    }

    #[test]
    fn clearing_forgets_every_row_and_every_event_moves_the_spinner_on() {
        let mut statuses = Statuses::default();
        statuses.note(&event(Target::Update("skill:a".into()), Phase::Started));
        statuses.tick();

        statuses.clear();

        assert_eq!(statuses.of(&State::Installed("skill:a".into())), None);
        assert_eq!(statuses.ticks(), 2);
    }
}
