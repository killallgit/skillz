//! The popup that asks for the name of a profile saved from what the project has.

use ratatui::Frame;

use super::super::act::Act;
use crate::tui::theme::Theme;
use crate::tui::widgets::input::InputField;
use crate::tui::widgets::modal::{self, ModalSize};
use crate::tui::widgets::panel::Panel;

const SIZE: ModalSize = ModalSize {
    width: 44,
    height: 3,
};

#[derive(Debug, Clone)]
pub(super) struct Naming {
    pub field: InputField,
}

impl Naming {
    pub fn new() -> Self {
        Naming {
            field: InputField::new("name"),
        }
    }

    /// What the typed name asks for; nothing while it is blank.
    pub fn act(&self) -> Option<Act> {
        let name = self.field.value().trim().to_string();
        (!name.is_empty()).then_some(Act::SaveCurrent { name })
    }

    pub fn render(&self, frame: &mut Frame, theme: &Theme) {
        let inner = modal::open(frame, SIZE, Panel::new("Save the project as a profile"), theme);
        self.field.render(frame, inner, theme);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::KeyCode;

    use super::*;
    use crate::tui::widgets::testing::{key, keys};

    fn typed(text: &str) -> Naming {
        let mut naming = Naming::new();
        keys(text).into_iter().for_each(|k| {
            naming.field.handle(k);
        });
        naming
    }

    #[test]
    fn the_typed_name_is_trimmed_into_the_save_act() {
        assert_eq!(
            typed(" snap ").act(),
            Some(Act::SaveCurrent {
                name: "snap".into()
            })
        );
    }

    #[test]
    fn a_blank_name_asks_for_nothing() {
        let mut naming = typed("  ");
        naming.field.handle(key(KeyCode::Enter));

        assert_eq!(naming.act(), None);
    }
}
