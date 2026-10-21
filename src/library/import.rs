use super::catalog::Catalog;
use crate::model::Augmentation;
use crate::snapshot::{Skipped, snapshot};

/// Add everything the snapshot collectors keep from the installed `items`; what they leave out
/// (hand-made, or needing secrets) is returned and stays out.
pub fn import_installed(catalog: &mut Catalog, items: &[Augmentation]) -> Vec<Skipped> {
    let snap = snapshot("library", items);
    snap.profile
        .entries
        .into_iter()
        .for_each(|entry| catalog.add_entry(entry));
    snap.skipped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Agent, Installer, Kind, Scope};
    use crate::snapshot::Reason;

    fn installed_skill(name: &str, source: Option<&str>) -> Augmentation {
        Augmentation {
            hooks: vec![],
            id: format!("skill:{name}"),
            name: name.into(),
            kind: Kind::Skill,
            agents: vec![Agent::Claude, Agent::Codex],
            scope: Scope::Project,
            group: source.unwrap_or("local").into(),
            source: source.map(Into::into),
            installer: Installer::Npx,
            children: vec![],
            removal: vec![],
        }
    }

    #[test]
    fn importing_adds_what_has_a_source_and_reports_the_rest_skipped() {
        let mut catalog = Catalog::default();
        let items = [
            installed_skill("tdd", Some("o/r")),
            installed_skill("mine", None),
        ];

        let skipped = import_installed(&mut catalog, &items);

        // Assert one item kept and the sourceless one skipped for having no source.
        assert_eq!(catalog.items().len(), 1);
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0].reason, Reason::NoSource);
    }

    #[test]
    fn importing_twice_does_not_duplicate() {
        let mut catalog = Catalog::default();
        let items = [installed_skill("tdd", Some("o/r"))];

        import_installed(&mut catalog, &items);
        import_installed(&mut catalog, &items);

        assert_eq!(catalog.items().len(), 1);
    }
}
