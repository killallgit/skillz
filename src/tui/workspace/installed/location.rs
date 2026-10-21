//! Where an installed item lives on disk, so it is clear why it counts for its group.

use crate::action::Action;
use crate::model::{Augmentation, Kind};

const SKILL_PREFIX: &str = "skill:";

/// The skill's directory, else the first file or directory its removal touches.
pub(super) fn location(item: &Augmentation) -> String {
    match item.kind {
        Kind::Skill => skill_dir(&item.id),
        _ => item.removal.iter().find_map(touched).unwrap_or_default(),
    }
}

/// Skill ids are the directory, plus a scope suffix when a project links to a global skill.
fn skill_dir(id: &str) -> String {
    let path = id.strip_prefix(SKILL_PREFIX).unwrap_or(id);
    let without_scope = [":project", ":global"]
        .into_iter()
        .find_map(|suffix| path.strip_suffix(suffix));
    without_scope.unwrap_or(path).to_string()
}

fn touched(step: &Action) -> Option<String> {
    match step {
        Action::RemovePath(target) => Some(target.path().display().to_string()),
        Action::Edit { file, .. } => Some(file.display().to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::rows::tests::installed;
    use super::*;

    #[test]
    fn a_skill_is_where_its_id_says_without_the_scope_suffix() {
        let mut plain = installed("a", Kind::Skill, None);
        plain.id = "skill:/app/.agents/skills/a".into();
        let mut linked = plain.clone();
        linked.id = "skill:/app/.agents/skills/a:project".into();

        let locations = [location(&plain), location(&linked)];

        assert_eq!(locations, ["/app/.agents/skills/a"; 2]);
    }

    #[test]
    fn an_item_that_removes_nothing_has_no_location() {
        let hook = installed("h", Kind::Hook, None);

        assert_eq!(location(&hook), "");
    }
}
