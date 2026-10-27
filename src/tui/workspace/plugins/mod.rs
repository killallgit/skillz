//! The plugins list: one flat list of the packages the project can have, each checked while the
//! project has it. Checking one installs it, unchecking removes it. Nothing is stored here: the
//! rows are derived from the agents' own config and the library each time a snapshot loads.

mod draw;
mod info;
mod keys;
mod rows;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

use self::rows::{Row, build};
use super::tab::Tab;
use crate::app::package::Package;
use crate::app::workspace::Act;
use crate::app::workspace::Model;
use crate::tui::keymap::{Action, Scope};
use crate::tui::theme::Theme;
use crate::tui::widgets::input::InputField;
use crate::tui::widgets::outline::Outline;
use crate::tui::widgets::text_modal::TextModal;
use skillz_harness::model::{Agent, Scope as Level};

/// The info modal over the list: a package's name and what is known of it.
struct Info {
    title: String,
    lines: Vec<String>,
    modal: TextModal,
}

pub struct PluginsTab {
    /// The package whose info is shown, when it is.
    info: Option<Info>,
    /// Whose installs the list shows and changes: the project's, or the user's alone.
    scope: Level,
    rows: Vec<Row>,
    outline: Outline,
    /// The packages flipped and not yet applied, each with whether it was installed when flipped.
    pending: HashMap<Package, bool>,
    /// Whether the filter is being typed.
    filtering: bool,
    /// The tools an install is made for.
    agents: Vec<Agent>,
}

impl PluginsTab {
    pub fn new(scope: Level) -> Self {
        PluginsTab {
            info: None,
            scope,
            pending: HashMap::new(),
            rows: vec![],
            outline: Outline::new(vec![]),
            filtering: false,
            agents: vec![],
        }
    }
}

impl Tab for PluginsTab {
    fn title(&self) -> &'static str {
        "Plugins"
    }

    fn scopes(&self) -> &'static [Scope] {
        &[Scope::Plugins]
    }

    fn load(&mut self, model: &Model) {
        self.agents.clone_from(&model.agents);
        self.rows = build(model, self.scope);
        // A flip stands while its package is still as it was; once it changed, the flip is done.
        let rows = &self.rows;
        self.pending.retain(|package, was| {
            rows.iter().any(|row| match row {
                Row::Own(plugin) => plugin.package == *package && plugin.is_installed() == *was,
                _ => false,
            })
        });
        self.outline
            .reshape(self.rows.iter().map(Row::shape).collect());
        self.settle(Action::Down);
    }

    fn handle(&mut self, key: KeyEvent) -> Option<Act> {
        self.handle_key(key)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        self.draw_body(frame, area, theme);
    }

    fn captures_keys(&self) -> bool {
        self.filtering || self.info.is_some()
    }

    fn prompt(&self) -> Option<&InputField> {
        self.filtering.then(|| self.outline.field())
    }
}
