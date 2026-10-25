use super::*;
use crate::stale::Disk;
use skillz_harness::Spec;

fn build(catalog: &Catalog) -> Vec<Row> {
    super::build(catalog, &Held::of(&[], &[]), &Disk)
}

pub fn skill(source: &str, name: &str) -> LibraryItem {
    LibraryItem::from_entry(Spec::Skill {
        source: source.into(),
        names: vec![name.into()],
        agents: vec![],
    })
}

pub fn hook(command: &str) -> LibraryItem {
    LibraryItem::from_entry(Spec::Hook {
        event: "Stop".into(),
        matcher: None,
        command: command.into(),
        agents: vec![],
    })
}

fn plugin(marketplace: &str, name: &str) -> LibraryItem {
    LibraryItem::from_entry(Spec::Plugin {
        source: format!("o/{marketplace}"),
        marketplace: marketplace.into(),
        name: name.into(),
        agents: vec![],
    })
}

fn packages(rows: &[Row]) -> Vec<Package> {
    rows.iter()
        .filter_map(|row| match row {
            Row::Package { package, .. } => Some(package.clone()),
            Row::Item(_) => None,
        })
        .collect()
}

#[test]
fn items_hang_under_the_package_they_come_from() {
    let catalog = Catalog::from_items(vec![hook("x"), skill("o/r", "b"), skill("o/r", "a")]);

    let rows = build(&catalog);

    let parents: Vec<Option<usize>> = shape(&rows).into_iter().map(|(p, _)| p).collect();
    assert_eq!(parents, [None, Some(0), Some(0), None, Some(3)]);
    assert_eq!(
        packages(&rows),
        [Package::Source("o/r".into()), Package::Hooks]
    );
}

#[test]
fn a_plugin_sits_with_the_skills_of_the_repo_hosting_its_marketplace() {
    let catalog = Catalog::from_items(vec![
        skill("o/mp", "a"),
        plugin("mp", "p"),
        plugin("mp", "q"),
        skill("o/r", "b"),
    ]);

    let rows = build(&catalog);

    assert_eq!(
        packages(&rows),
        [
            Package::Source("o/mp".into()),
            Package::Source("o/r".into())
        ]
    );
}

#[test]
fn an_empty_library_has_no_rows() {
    assert!(build(&Catalog::default()).is_empty());
}
