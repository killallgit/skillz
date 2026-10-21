//! The popup that asks for a profile name, for a new profile or for one saved from the project.

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Purpose {
    New,
    SaveCurrent,
}

#[derive(Debug, Clone)]
pub(super) struct Naming {
    purpose: Purpose,
    pub field: InputField,
}

impl Naming {
    pub fn new(purpose: Purpose) -> Self {
        Naming {
            purpose,
            field: InputField::new("name"),
        }
    }

    /// What the typed name asks for; nothing while it is blank.
    pub fn act(&self) -> Option<Act> {
        let name = self.field.value().trim().to_string();
        match (name.is_empty(), self.purpose) {
            (true, _) => None,
            (false, Purpose::New) => Some(Act::NewProfile { name }),
            (false, Purpose::SaveCurrent) => Some(Act::SaveCurrent { name }),
        }
    }

    pub fn render(&self, frame: &mut Frame, theme: &Theme) {
        let title = match self.purpose {
            Purpose::New => "New profile",
            Purpose::SaveCurrent => "Save the project as a profile",
        };
        let inner = modal::open(frame, SIZE, Panel::new(title), theme);
        self.field.render(frame, inner, theme);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::KeyCode;

    use super::*;
    use crate::tui::widgets::testing::{key, keys};

    fn typed(purpose: Purpose, text: &str) -> Naming {
        let mut naming = Naming::new(purpose);
        keys(text).into_iter().for_each(|k| {
            naming.field.handle(k);
        });
        naming
    }

    #[test]
    fn the_purpose_decides_which_act_the_name_asks_for() {
        let new = typed(Purpose::New, " web ").act();
        let save = typed(Purpose::SaveCurrent, "snap").act();

        assert_eq!(new, Some(Act::NewProfile { name: "web".into() }));
        assert_eq!(
            save,
            Some(Act::SaveCurrent {
                name: "snap".into()
            })
        );
    }

    #[test]
    fn a_blank_name_asks_for_nothing() {
        let mut naming = typed(Purpose::New, "  ");
        naming.field.handle(key(KeyCode::Enter));

        assert_eq!(naming.act(), None);
    }
}
