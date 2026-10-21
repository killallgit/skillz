//! A user-level catalog of available augmentations that profiles are built from. Items are
//! copied into profiles; a profile never refers back to the library.

// Nothing consumes the library until the TUI tab lands (skillz-9gs.3); remove then.
#![allow(dead_code, unused_imports)]

mod catalog;
mod import;
mod item;
mod store;

pub(crate) use catalog::Catalog;
pub(crate) use import::import_installed;
pub(crate) use item::{ItemKind, LibraryItem};
pub(crate) use store::{Corrupt, Store};
