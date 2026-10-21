//! The ids of items the background update check has found newer versions of, kept for the
//! session. Results arrive on a channel and are taken whenever the screen is drawn.

use std::collections::BTreeSet;
use std::sync::mpsc::Receiver;

#[derive(Default)]
pub(super) struct Checks {
    found: Option<Receiver<String>>,
    outdated: BTreeSet<String>,
}

impl Checks {
    pub fn watch(&mut self, found: Receiver<String>) {
        self.found = Some(found);
    }

    /// Everything found so far.
    pub fn outdated(&mut self) -> &BTreeSet<String> {
        if let Some(found) = &self.found {
            self.outdated.extend(found.try_iter());
        }
        &self.outdated
    }

    pub fn forget(&mut self, ids: &[String]) {
        self.outdated.retain(|id| !ids.contains(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn found_ids_are_kept_until_forgotten() {
        let (tx, rx) = mpsc::channel();
        let mut checks = Checks::default();
        checks.watch(rx);

        tx.send("a".to_string()).unwrap();
        tx.send("b".to_string()).unwrap();
        let found = checks.outdated().len();
        checks.forget(&["a".to_string()]);

        assert_eq!((found, checks.outdated().len()), (2, 1));
    }
}
