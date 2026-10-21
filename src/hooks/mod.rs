//! Hook handlers in agent config files: reading, grouping by tool, and removing them.

mod discover;
mod group;
mod layout;
#[cfg(test)]
mod testutil;

use std::path::Path;

pub use discover::discover;
pub use group::hook_group;
pub use layout::{parse, remove_hook};

/// One hook handler as written in a hooks config file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookEntry {
    pub event: String,
    pub matcher: Option<String>,
    pub command: String,
}

/// Syntax of a config file, decided by its extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Toml,
}

impl Format {
    /// `.toml` files are TOML; everything else is JSON.
    pub fn of(file: &Path) -> Self {
        if file.extension().is_some_and(|e| e == "toml") {
            Format::Toml
        } else {
            Format::Json
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_from_extension() {
        assert_eq!(Format::of(Path::new("a/config.toml")), Format::Toml);
        assert_eq!(Format::of(Path::new("a/hooks.json")), Format::Json);
        assert_eq!(Format::of(Path::new("a/toml")), Format::Json);
    }
}
