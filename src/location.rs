//! Where an installed item lives on disk, so it is clear why it counts for its group.

use crate::action::Action;
use crate::merge::tag;
use crate::model::{Augmentation, Kind};

const SKILL_PREFIX: &str = "skill:";

/// The skill's directory, else the first file or directory its removal touches.
pub fn location(item: &Augmentation) -> String {
    match item.kind {
        Kind::Skill => skill_dir(&item.id),
        _ => item.removal.iter().find_map(touched).unwrap_or_default(),
    }
}

/// One `agents: location` per copy of an item seen through several agents; none for a lone item.
pub fn places(item: &Augmentation) -> Vec<String> {
    item.members
        .iter()
        .map(|m| format!("{}: {}", tag(&m.agents), location(m)))
        .collect()
}

/// What to show beside an item: its location, or every copy's with its agents.
pub fn detail(item: &Augmentation) -> String {
    match places(item) {
        places if places.is_empty() => location(item),
        places => places.join("; "),
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
    use super::*;
    use crate::model::{Agent, Installer, Scope};

    fn installed(name: &str, kind: Kind, source: Option<&str>) -> Augmentation {
        Augmentation {
            hooks: vec![],
            members: Vec::new(),
            id: format!("{kind}:{name}"),
            name: name.into(),
            kind,
            agents: vec![Agent::Claude],
            scope: Scope::Project,
            group: source.unwrap_or("local").into(),
            source: source.map(Into::into),
            installer: Installer::Npx,
            origin: Default::default(),
            children: vec![],
            removal: vec![],
        }
    }

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
    fn a_merged_item_shows_each_copys_agents_and_location() {
        let copy = |agent, path: &str| Augmentation {
            agents: vec![agent],
            id: format!("skill:{path}"),
            ..installed("a", Kind::Skill, None)
        };
        let merged = Augmentation {
            members: vec![copy(Agent::Claude, "/c/a"), copy(Agent::Codex, "/x/a")],
            ..installed("a", Kind::Skill, None)
        };

        assert_eq!(places(&merged), ["claude: /c/a", "codex: /x/a"]);
        assert_eq!(detail(&merged), "claude: /c/a; codex: /x/a");
    }

    #[test]
    fn a_lone_item_shows_only_its_location() {
        let mut lone = installed("a", Kind::Skill, None);
        lone.id = "skill:/c/a".into();

        assert_eq!(detail(&lone), "/c/a");
        assert!(places(&lone).is_empty());
    }

    #[test]
    fn an_item_that_removes_nothing_has_no_location() {
        let hook = installed("h", Kind::Hook, None);

        assert_eq!(location(&hook), "");
    }
}
