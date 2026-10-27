//! Where an installed item lives on disk, so it is clear why it counts for its group.

use skillz_harness::model::Installed;

/// The location discovery recorded for this installed item.
pub fn location(item: &Installed) -> String {
    item.details
        .path()
        .map(|path| path.display().to_string())
        .unwrap_or_default()
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
