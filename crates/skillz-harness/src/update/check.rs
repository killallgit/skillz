//! Finding out, without changing anything, whether an item has a newer version. Only what can
//! be told reliably is checked; every other kind reads as having no update.

use crate::model::Installed;

pub trait UpdateCheck: Send + Sync {
    fn available(&self, item: &Installed) -> bool;
    /// Whether `available` is a real answer for `item` rather than the default of no update.
    fn can_tell(&self, item: &Installed) -> bool;
}

/// Reports no update for anything: the check is switched off.
pub struct NoCheck;

impl UpdateCheck for NoCheck {
    fn available(&self, _item: &Installed) -> bool {
        false
    }

    fn can_tell(&self, _item: &Installed) -> bool {
        false
    }
}

/// Whether any copy of `item` has a newer version.
pub fn any_available(checker: &dyn UpdateCheck, item: &Installed) -> bool {
    item.parts().iter().any(|part| checker.available(part))
}
