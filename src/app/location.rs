//! Where an installed item lives on disk, so it is clear why it counts for its group.

use crate::app::merge::tag;
use skillz_harness::model::Installed;

/// The location discovery recorded for this installed item.
pub fn location(item: &Installed) -> String {
    item.details
        .path()
        .map(|path| path.display().to_string())
        .unwrap_or_default()
}

/// One `agents: location` per copy of an item seen through several agents; none for a lone item.
pub fn places(item: &Installed) -> Vec<String> {
    item.members
        .iter()
        .map(|m| format!("{}: {}", tag(&m.agents), location(m)))
        .collect()
}

/// What to show beside an item: its location, or every copy's with its agents.
pub fn detail(item: &Installed) -> String {
    match places(item) {
        places if places.is_empty() => location(item),
        places => places.join("; "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skillz_harness::model::{Agent, Details, Installed, Installer, Kind, Scope};

    #[test]
    fn location_comes_from_installed_metadata_even_when_the_id_has_a_scope_suffix() {
        let path = std::path::PathBuf::from("/app/skills/name:project");
        let item = Installed {
            details: Details::Skill {
                directory: path.clone(),
                install_name: "name".into(),
            },
            id: "opaque:project".into(),
            name: "name".into(),
            kind: Kind::Skill,
            agents: vec![Agent::Claude],
            scope: Scope::Project,
            group: "local".into(),
            source: None,
            installer: Installer::Local,
            origin: Default::default(),
            children: vec![],
            hooks: vec![],
            members: vec![],
            removal: vec![],
        };
        assert_eq!(std::path::Path::new(&location(&item)), path.as_path());
    }
}
