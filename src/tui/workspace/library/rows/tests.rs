use super::*;
use crate::profile::Entry;

pub fn skill(source: &str, name: &str) -> LibraryItem {
    LibraryItem::from_entry(Entry::Skill {
        source: source.into(),
        names: vec![name.into()],
        agents: vec![],
    })
}

pub fn hook(command: &str) -> LibraryItem {
    LibraryItem::from_entry(Entry::Hook {
        event: "Stop".into(),
        matcher: None,
        command: command.into(),
        agents: vec![],
    })
}

fn plugin(marketplace: &str, name: &str) -> LibraryItem {
    LibraryItem::from_entry(Entry::Plugin {
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
fn skills_and_a_marketplace_plugin_are_separate_packages_with_their_counts() {
    let catalog = Catalog::from_items(vec![
        skill("o/r", "a"),
        plugin("mp", "p"),
        plugin("mp", "q"),
    ]);

    let rows = build(&catalog);

    assert_eq!(
        packages(&rows),
        [
            Package::Marketplace("mp".into()),
            Package::Source("o/r".into())
        ]
    );
    assert!(matches!(
        &rows[0],
        Row::Package { contents, .. } if *contents == [(ItemKind::Plugin, 2)]
    ));
}

#[test]
fn an_empty_library_has_no_rows() {
    assert!(build(&Catalog::default()).is_empty());
}
