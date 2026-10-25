//! Reading and rewriting hook handlers in the text of an agent config file. Pure text in, text
//! out, so `action` can apply these edits without depending on hook discovery.

mod add;
mod layout;
#[cfg(test)]
pub mod testutil;

use std::path::Path;

pub use add::add_hook;
pub use layout::{parse, remove_hook};

/// One hook handler as written in a hooks config file.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct HookEntry {
    pub event: String,
    pub matcher: Option<String>,
    pub command: String,
}

impl HookEntry {
    /// `Event [matcher]: command`, as shown under whatever owns the hook.
    pub fn label(&self) -> String {
        match &self.matcher {
            Some(m) => format!("{} [{}]: {}", self.event, m, self.command),
            None => format!("{}: {}", self.event, self.command),
        }
    }
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
