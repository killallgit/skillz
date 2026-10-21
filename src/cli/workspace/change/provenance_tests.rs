use std::path::Path;

use super::tests::{session, skill_entry};
use super::*;
use crate::agents::registry;
use crate::install::test_support::{FakeNpx, LOCKED_LINK, ScriptedPrompter};
use crate::library::{LibraryItem, Provenance};
use crate::model::Installer;

fn install_picked(dir: &Path, library: &[LibraryItem]) -> (FakeNpx, Store) {
    let runner = FakeNpx::new();
    let prompter = ScriptedPrompter::new(&[], true);
    let deps = Deps::for_test(dir, registry(), (&runner, &prompter));
    let store = Store::in_env(&project_env(&deps, dir));
    store
        .update(|document| {
            library
                .iter()
                .for_each(|item| document.items.add(item.clone()));
            Ok(())
        })
        .unwrap();
    let entries = [skill_entry("tdd")];
    let picked = Selection {
        install: &entries,
        remove: &[],
    };
    change(
        &session(&deps, dir),
        Scope::Project,
        &Change::Selection(&picked),
    )
    .unwrap();
    (runner, store)
}

#[test]
fn installing_records_where_the_item_came_from() {
    let dir = tempfile::tempdir().unwrap();

    let (_, store) = install_picked(dir.path(), &[]);

    // Assert the lock's link, hash and the installer landed in the library.
    let items = store.read().unwrap().items;
    let provenance = items.items()[0].provenance.clone().unwrap();
    assert_eq!(provenance.link.as_deref(), Some(LOCKED_LINK));
    assert_eq!(provenance.version.as_deref(), Some("abc123"));
    assert_eq!(provenance.engine, Installer::Npx);
}

#[test]
fn installing_again_fetches_from_the_recorded_link() {
    let dir = tempfile::tempdir().unwrap();
    let known = LibraryItem::from_entry(skill_entry("tdd")).with_provenance(Provenance {
        link: Some("https://elsewhere.test/o/r.git".into()),
        engine: Installer::Npx,
        version: None,
        recorded_at: 1,
    });

    let (runner, _) = install_picked(dir.path(), &[known]);

    // Assert npx was given the recorded link rather than the entry's own source.
    let args = runner.calls.borrow()[0].clone();
    assert!(args.contains(&"https://elsewhere.test/o/r.git".to_string()));
    assert!(!args.contains(&"o/r".to_string()));
}
