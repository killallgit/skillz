use super::catalog::Catalog;
use super::item::LibraryItem;
use super::provenance::Provenance;
use crate::snapshot::{Skipped, collect};
use skillz_harness::model::Installed;

/// Reinstallable entries from the installed `items`, each with the provenance its
/// installer left; what they leave out (needing secrets, or with nothing to reinstall from) is
/// returned apart.
pub(super) fn collect_installed(
    items: &[Installed],
    recorded_at: u64,
) -> (Vec<LibraryItem>, Vec<Skipped>) {
    let mut kept = vec![];
    let mut skipped = vec![];
    for item in items {
        let (entries, left_out) = collect(std::slice::from_ref(item));
        skipped.extend(left_out);
        for entry in entries {
            for each in LibraryItem::each_of(entry) {
                kept.push(each.with_provenance(Provenance::of(item, recorded_at)));
            }
        }
    }
    (kept, skipped)
}

/// Add every reinstallable entry from the installed `items`.
pub fn import_installed(catalog: &mut Catalog, items: &[Installed], recorded_at: u64) {
    let (kept, _) = collect_installed(items, recorded_at);
    kept.into_iter().for_each(|item| catalog.add(item));
}

/// Add what an install left behind: the items in `after` that were not in `before` or were
/// changed since.
pub fn record_installed(
    catalog: &mut Catalog,
    (before, after): (&[Installed], &[Installed]),
    recorded_at: u64,
) {
    let fresh: Vec<Installed> = after
        .iter()
        .filter(|item| !before.contains(item))
        .cloned()
        .collect();
    import_installed(catalog, &fresh, recorded_at);
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use skillz_harness::model::{Agent, Installer, Kind, Origin, Scope};

    pub fn installed_skill(name: &str, source: Option<&str>) -> Installed {
        Installed {
            details: Default::default(),
            hooks: vec![],
            members: Vec::new(),
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
    fn importing_adds_what_has_a_source() {
        let mut catalog = Catalog::default();
        let items = [
            installed_skill("tdd", Some("o/r")),
            installed_skill("mine", None),
        ];

        import_installed(&mut catalog, &items, 7);

        assert_eq!(catalog.items().len(), 1);
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

        assert_eq!(
            catalog.items()[0].provenance,
            Some(Provenance {
                link: Some("https://h/o/r.git".into()),
                engine: Installer::Npx,
                version: Some("abc".into()),
                recorded_at: 7,
                detected: false,
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

        assert_eq!(catalog.items().len(), 1);
    }
}
