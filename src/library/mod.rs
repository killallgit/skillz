//! A user-level catalog of available augmentations that profiles are built from. Items are
//! copied into profiles; a profile never refers back to the library.

mod catalog;
mod import;
mod item;
mod store;

pub use catalog::Catalog;
pub use import::import_installed;
pub use item::{ItemKind, LibraryItem};
pub use store::{Corrupt, Store};
