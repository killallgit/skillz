//! What the background library sync has to say, taken whenever the screen is drawn.

use std::sync::mpsc::Receiver;

use crate::workspace::Synced;

#[derive(Default)]
pub(super) struct Syncs {
    done: Option<Receiver<Synced>>,
}

impl Syncs {
    pub fn watch(&mut self, done: Receiver<Synced>) {
        self.done = Some(done);
    }

    /// The syncs that finished since the last call.
    pub fn finished(&self) -> Vec<Synced> {
        self.done
            .as_ref()
            .map(|done| done.try_iter().collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn a_finished_sync_is_taken_once() {
        let (tx, rx) = mpsc::channel();
        let mut syncs = Syncs::default();
        syncs.watch(rx);
        tx.send(Synced::default()).unwrap();

        let first = syncs.finished().len();
        let second = syncs.finished().len();

        assert_eq!((first, second), (1, 0));
    }

    #[test]
    fn nothing_finishes_without_a_watch() {
        assert!(Syncs::default().finished().is_empty());
    }
}
