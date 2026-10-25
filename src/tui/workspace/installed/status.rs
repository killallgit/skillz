//! Where each row's change stands while it runs: working, done or failed. Held only until the
//! next snapshot or key, so a finished change is never shown for longer than the screen it
//! belongs to.

use crate::app::review::{ItemEvent, Phase, Target};

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

    /// The status of the item with `id`, when a change is working on it.
    pub fn of(&self, id: &str) -> Option<Status> {
        self.targets
            .iter()
            .find(|(target, _)| concerns(target, id))
            .map(|(_, status)| *status)
    }
}

fn concerns(target: &Target, id: &str) -> bool {
    matches!(target, Target::Remove(row) | Target::Update(row) if row == id)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let target = Target::Remove("skill:a".into());
        let row = "skill:a";

        statuses.note(&event(target.clone(), Phase::Started));
        let working = statuses.of(row);
        statuses.note(&event(target.clone(), Phase::Succeeded));
        let done = statuses.of(row);
        statuses.note(&event(target, Phase::Failed("no".into())));
        let failed = statuses.of(row);

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

        let named = statuses.of("skill:a");
        let other = statuses.of("skill:b");

        assert_eq!((named, other), (Some(Status::Working), None));
    }

    #[test]
    fn clearing_forgets_every_row_and_every_event_moves_the_spinner_on() {
        let mut statuses = Statuses::default();
        statuses.note(&event(Target::Update("skill:a".into()), Phase::Started));
        statuses.tick();

        statuses.clear();

        assert_eq!(statuses.of("skill:a"), None);
        assert_eq!(statuses.ticks(), 2);
    }
}
