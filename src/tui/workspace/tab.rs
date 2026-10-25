//! What a tab is to the workspace. A tab owns its state, takes the keys the workspace does not,
//! and draws into the body; the tab bar, key bar, help and toasts belong to the workspace.

use std::collections::BTreeSet;

use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

use crate::review::ItemEvent;
use crate::tui::keymap::Scope;
use crate::tui::theme::Theme;
use crate::tui::widgets::input::InputField;
use crate::workspace::Act;
use crate::workspace::Model;

pub trait Tab: Send {
    fn title(&self) -> &'static str;

    /// The tab's own key scopes, for the key bar and `?`; the workspace adds the tab keys.
    fn scopes(&self) -> &'static [Scope] {
        &[]
    }

    /// Take the new snapshot, keeping the cursor, open rows and filter where they were.
    fn load(&mut self, model: &Model);

    fn handle(&mut self, key: KeyEvent) -> Option<Act>;

    fn draw(&mut self, frame: &mut Frame, area: Rect, theme: &Theme);

    /// Ids of installed items found to have an update; a tab may mark them.
    fn mark_outdated(&mut self, _ids: &BTreeSet<String>) {}

    /// An item's change started or ended; a tab may show where it stands.
    fn note(&mut self, _event: &ItemEvent) {}

    /// Time has passed while a change runs; animations move on.
    fn tick(&mut self) {}

    /// A text field or popup wants every key, tab keys included.
    fn captures_keys(&self) -> bool {
        false
    }

    /// The field being typed into, drawn where the key bar usually is.
    fn prompt(&self) -> Option<&InputField> {
        None
    }
}
