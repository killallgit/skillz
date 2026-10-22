//! Finding out, without changing anything, whether an item has a newer version. Only what can
//! be told reliably is checked; every other kind reads as having no update.

use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread;

use crate::model::Augmentation;

pub trait UpdateCheck: Send + Sync {
    fn available(&self, item: &Augmentation) -> bool;
}

/// Reports no update for anything: the check is switched off.
pub struct NoCheck;

impl UpdateCheck for NoCheck {
    fn available(&self, _item: &Augmentation) -> bool {
        false
    }
}

/// Whether any copy of `item` has a newer version.
pub fn any_available(checker: &dyn UpdateCheck, item: &Augmentation) -> bool {
    item.parts().iter().any(|part| checker.available(part))
}

/// Check `items` on a thread of their own, sending the id of each that has an update as it is
/// found. Stops when the receiver is gone.
pub fn spawn(checker: Arc<dyn UpdateCheck>, items: Vec<Augmentation>, found: Sender<String>) {
    thread::spawn(move || {
        for item in items {
            if any_available(checker.as_ref(), &item) && found.send(item.id).is_err() {
                return;
            }
        }
    });
}
