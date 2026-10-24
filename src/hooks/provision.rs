use std::path::PathBuf;

use crate::action::{Action, Edit};
use crate::profile::Spec;

/// The step that adds a profile's hook to `file`. `None` for other kinds of entry.
pub fn provision(entry: &Spec, file: PathBuf) -> Option<Vec<Action>> {
    let Spec::Hook {
        event,
        matcher,
        command,
        ..
    } = entry
    else {
        return None;
    };
    Some(vec![Action::Edit {
        file,
        edit: Edit::AddHook {
            event: event.clone(),
            matcher: matcher.clone(),
            command: command.clone(),
        },
    }])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hook() -> Spec {
        Spec::Hook {
            event: "Stop".into(),
            matcher: None,
            command: "say done".into(),
            agents: vec![],
        }
    }

    #[test]
    fn a_hook_entry_becomes_one_edit_of_the_given_file() {
        let steps = provision(&hook(), PathBuf::from("/c/settings.json")).unwrap();

        assert!(matches!(
            steps.as_slice(),
            [Action::Edit { file, edit: Edit::AddHook { event, .. } }]
                if file == &PathBuf::from("/c/settings.json") && event == "Stop"
        ));
    }

    #[test]
    fn other_entries_are_not_hooks() {
        let skill = Spec::Skill {
            source: "o/r".into(),
            names: vec!["a".into()],
            agents: vec![],
        };

        assert!(provision(&skill, PathBuf::new()).is_none());
    }
}
