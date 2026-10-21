use super::catalog::Catalog;
use super::item::LibraryItem;
use super::provenance::Provenance;
use crate::model::Augmentation;
use crate::snapshot::{Skipped, snapshot};

/// Add everything the snapshot collectors keep from the installed `items`, each with the
/// provenance its installer left; what they leave out (hand-made, or needing secrets) is
/// returned and stays out.
pub fn import_installed(
    catalog: &mut Catalog,
    items: &[Augmentation],
    recorded_at: u64,
) -> Vec<Skipped> {
    let mut skipped = vec![];
    for item in items {
        let snap = snapshot("library", std::slice::from_ref(item));
        skipped.extend(snap.skipped);
        for entry in snap.profile.entries {
            for each in LibraryItem::each_of(entry) {
                catalog.add(each.with_provenance(Provenance::of(item, recorded_at)));
            }
        }
    }
    skipped
}

/// Add what an install left behind: the items in `after` that were not in `before` or were
/// changed since.
pub fn record_installed(
    catalog: &mut Catalog,
    (before, after): (&[Augmentation], &[Augmentation]),
    recorded_at: u64,
) {
    let fresh: Vec<Augmentation> = after
        .iter()
        .filter(|item| !before.contains(item))
        .cloned()
        .collect();
    import_installed(catalog, &fresh, recorded_at);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Agent, Installer, Kind, Origin, Scope};
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
            origin: Origin {
                link: Some("https://h/o/r.git".into()),
                version: Some("abc".into()),
            },
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

        let skipped = import_installed(&mut catalog, &items, 7);

        // Assert one item kept and the sourceless one skipped for having no source.
        assert_eq!(catalog.items().len(), 1);
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0].reason, Reason::NoSource);
    }

    #[test]
    fn importing_twice_does_not_duplicate() {
        let mut catalog = Catalog::default();
        let items = [installed_skill("tdd", Some("o/r"))];

        import_installed(&mut catalog, &items, 7);
        import_installed(&mut catalog, &items, 8);

        assert_eq!(catalog.items().len(), 1);
    }

    #[test]
    fn an_imported_item_remembers_how_it_was_installed() {
        let mut catalog = Catalog::default();

        import_installed(&mut catalog, &[installed_skill("tdd", Some("o/r"))], 7);

        // Assert the link, engine, version and time came from the installed item.
        assert_eq!(
            catalog.items()[0].provenance,
            Some(Provenance {
                link: Some("https://h/o/r.git".into()),
                engine: Installer::Npx,
                version: Some("abc".into()),
                recorded_at: 7,
            })
        );
    }

    #[test]
    fn recording_an_install_adds_only_what_is_new_or_changed() {
        let mut catalog = Catalog::default();
        let before = [installed_skill("old", Some("o/r"))];
        let after = [
            installed_skill("old", Some("o/r")),
            installed_skill("new", Some("o/r")),
        ];

        record_installed(&mut catalog, (&before, &after), 7);

        // Assert the untouched skill stays out of the library.
        assert_eq!(catalog.items().len(), 1);
    }
}
