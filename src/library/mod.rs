//! A user-level catalog of available augmentations that profiles are built from, each with
//! where it came from. Items are copied into profiles; a profile never refers back to the
//! library. The catalog is persisted in the `store`.

mod catalog;
mod import;
mod item;
mod provenance;
mod sync;

pub use catalog::Catalog;
pub use import::{import_installed, record_installed};
pub use item::{ItemKind, LibraryItem};
pub use provenance::{Provenance, now_secs};
pub use sync::{Synced, sync_installed};
